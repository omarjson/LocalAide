use crate::app::error::AppError;
use crate::domain::document::SearchResultDoc;
use crate::services::library::db::{FtsHit, LibraryDb};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Keyword,
    Semantic,
    Hybrid,
}

pub struct HybridSearcher {
    db: Arc<LibraryDb>,
}

impl HybridSearcher {
    pub fn new(db: Arc<LibraryDb>) -> Self {
        Self { db }
    }

    pub fn search(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
        mode: SearchMode,
        limit: usize,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        match mode {
            SearchMode::Keyword => self.search_keyword(query, limit),
            SearchMode::Semantic => {
                if let Some(vec) = query_vector {
                    self.search_vector(vec, limit)
                } else {
                    // Fallback to keyword if no vector provided
                    self.search_keyword(query, limit)
                }
            }
            SearchMode::Hybrid => {
                let keyword_hits = self.db.fts_search(query, limit * 2).unwrap_or_default();
                let vector_hits = if let Some(vec) = query_vector {
                    self.search_vector_raw(vec, limit * 2).unwrap_or_default()
                } else {
                    Vec::new()
                };

                if vector_hits.is_empty() {
                    return Ok(Self::map_fts_to_docs(keyword_hits, limit));
                }

                Ok(Self::reciprocal_rank_fusion(
                    keyword_hits,
                    vector_hits,
                    limit,
                ))
            }
        }
    }

    fn search_keyword(&self, query: &str, limit: usize) -> Result<Vec<SearchResultDoc>, AppError> {
        let hits = self.db.fts_search(query, limit)?;
        Ok(Self::map_fts_to_docs(hits, limit))
    }

    fn search_vector(
        &self,
        query_vec: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        let hits = self.search_vector_raw(query_vec, limit)?;
        let docs = hits
            .into_iter()
            .take(limit)
            .map(|hit| SearchResultDoc {
                path: hit.path,
                title: hit.title,
                snippet: hit.snippet,
                score: hit.score,
                modified: hit.modified,
                size: hit.size,
            })
            .collect();
        Ok(docs)
    }

    fn search_vector_raw(&self, query_vec: &[f32], limit: usize) -> Result<Vec<RawHit>, AppError> {
        // Score every stored vector, but keep only the best `limit`. Text is
        // read afterwards, for the winners only.
        let mut best: Vec<(f32, String)> = Vec::with_capacity(limit + 1);
        self.db.visit_chunk_embeddings(|chunk_id, embedding| {
            keep_best(&mut best, limit, cosine_similarity(query_vec, embedding), chunk_id);
        })?;

        let scores: HashMap<String, f32> =
            best.iter().map(|(score, id)| (id.clone(), *score)).collect();
        let ids: Vec<String> = best.into_iter().map(|(_, id)| id).collect();
        let hits = self
            .db
            .chunk_details(&ids)?
            .into_iter()
            .map(|chunk| {
                let snippet = if chunk.text.chars().count() > 200 {
                    format!("{}...", chunk.text.chars().take(200).collect::<String>())
                } else {
                    chunk.text
                };
                RawHit {
                    score: scores.get(&chunk.chunk_id).copied().unwrap_or(0.0),
                    chunk_id: chunk.chunk_id,
                    path: chunk.path,
                    title: chunk.title,
                    snippet,
                    modified: chunk.modified,
                    size: chunk.size,
                }
            })
            .collect();
        Ok(hits)
    }

    fn map_fts_to_docs(hits: Vec<FtsHit>, limit: usize) -> Vec<SearchResultDoc> {
        const RRF_K: f32 = 60.0;
        hits.into_iter()
            .enumerate()
            .take(limit)
            .map(|(rank, h)| {
                let score = 1.0 / (RRF_K + rank as f32 + 1.0);
                SearchResultDoc {
                    path: h.path,
                    title: h.title,
                    snippet: h.snippet,
                    score,
                    modified: h.modified,
                    size: h.size,
                }
            })
            .collect()
    }

    fn reciprocal_rank_fusion(
        keyword_hits: Vec<FtsHit>,
        vector_hits: Vec<RawHit>,
        limit: usize,
    ) -> Vec<SearchResultDoc> {
        const RRF_K: f32 = 60.0;
        let mut rrf_scores: HashMap<String, f32> = HashMap::new();
        let mut doc_info: HashMap<String, (String, String, String, u64, u64)> = HashMap::new();

        // Score keyword ranks
        for (rank, hit) in keyword_hits.into_iter().enumerate() {
            let chunk_id = hit.chunk_id;
            let score = 1.0 / (RRF_K + rank as f32 + 1.0);
            *rrf_scores.entry(chunk_id.clone()).or_insert(0.0) += score;
            doc_info.entry(chunk_id).or_insert((
                hit.path,
                hit.title,
                hit.snippet,
                hit.modified,
                hit.size,
            ));
        }

        // Score vector ranks
        for (rank, hit) in vector_hits.into_iter().enumerate() {
            let chunk_id = hit.chunk_id;
            let score = 1.0 / (RRF_K + rank as f32 + 1.0);
            *rrf_scores.entry(chunk_id.clone()).or_insert(0.0) += score;
            doc_info.entry(chunk_id).or_insert((
                hit.path,
                hit.title,
                hit.snippet,
                hit.modified,
                hit.size,
            ));
        }

        // Deduplicate and rank
        let mut scored_items: Vec<(String, f32)> = rrf_scores.into_iter().collect();
        scored_items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut results = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        for (chunk_id, rrf_score) in scored_items {
            if let Some((path, title, snippet, modified, size)) = doc_info.remove(&chunk_id) {
                // Deduplicate chunks from the same document in top results if desired, or allow multiple
                let is_new_path = seen_paths.insert(path.clone());
                if is_new_path || results.len() < limit / 2 {
                    results.push(SearchResultDoc {
                        path,
                        title,
                        snippet,
                        score: rrf_score,
                        modified,
                        size,
                    });
                }
            }
            if results.len() >= limit {
                break;
            }
        }

        results
    }
}

#[derive(Debug, Clone)]
struct RawHit {
    chunk_id: String,
    path: String,
    title: String,
    snippet: String,
    score: f32,
    modified: u64,
    size: u64,
}

/// Adds `(score, id)` to `best`, which stays sorted from high to low and holds
/// at most `limit` entries. Scores that are not finite numbers are ignored.
fn keep_best(best: &mut Vec<(f32, String)>, limit: usize, score: f32, id: &str) {
    if !score.is_finite() {
        return;
    }
    let position = best.partition_point(|(kept, _)| *kept >= score);
    if position < limit {
        best.insert(position, (score, id.to_string()));
        best.truncate(limit);
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom > 1e-6 { dot / denom } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::keep_best;

    #[test]
    fn keep_best_holds_the_top_scores_in_order() {
        let mut best = Vec::new();
        let scored = [
            (0.2, "a"),
            (0.9, "b"),
            (f32::NAN, "not a number"),
            (0.5, "c"),
            (0.1, "d"),
            (0.7, "e"),
        ];
        for (score, id) in scored {
            keep_best(&mut best, 3, score, id);
        }
        let ids: Vec<&str> = best.iter().map(|(_, id)| id.as_str()).collect();
        assert_eq!(ids, ["b", "e", "c"]);

        let mut none = Vec::new();
        keep_best(&mut none, 0, 1.0, "x");
        assert!(none.is_empty());
    }
}
