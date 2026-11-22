use crate::app::error::AppError;
use crate::domain::document::SearchResultDoc;
use crate::services::library::db::{FtsHit, LibraryDb};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
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
        // read afterwards, for the winners only. A bounded heap avoids the
        // per-row `Vec::insert`+`truncate` of a sorted buffer.
        let mut best = TopK::new(limit);
        self.db.visit_chunk_embeddings(|chunk_id, embedding| {
            best.offer(cosine_similarity(query_vec, embedding), chunk_id);
        })?;
        let best = best.into_sorted();

        let scores: HashMap<String, f32> = best
            .iter()
            .map(|(score, id)| (id.clone(), *score))
            .collect();
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

/// One candidate in the bounded top-k heap. `Ord` is reversed so that the
/// max-heap's top is the *worst* kept item (lowest score, latest arrival),
/// which is the one evicted when a better candidate arrives.
#[derive(Debug)]
struct Candidate {
    score: f32,
    seq: u64,
    id: String,
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // `other.score.total_cmp(self.score)` is Greater when our score is
        // lower, i.e. when we are worse and should be nearer the heap top.
        match other.score.total_cmp(&self.score) {
            Ordering::Equal => self.seq.cmp(&other.seq),
            ordering => ordering,
        }
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

/// Keeps the `limit` highest-scoring ids seen so far, in a heap of bounded
/// size. Each offer is O(log limit) with no per-row shifting or truncation;
/// non-finite scores are ignored and equal scores keep the first arrivals.
pub struct TopK {
    limit: usize,
    heap: BinaryHeap<Candidate>,
    next_seq: u64,
}

impl TopK {
    pub fn new(limit: usize) -> Self {
        // Capacity is a bounded hint; the heap itself never exceeds `limit`.
        let capacity = limit.saturating_add(1).min(4096);
        Self {
            limit,
            heap: BinaryHeap::with_capacity(capacity),
            next_seq: 0,
        }
    }

    /// Offer one `(score, id)`. The id string is only allocated if the
    /// candidate makes the cut. New arrivals always have a later sequence
    /// number than kept ones, so an equal score never evicts an earlier tie.
    pub fn offer(&mut self, score: f32, id: &str) {
        if !score.is_finite() || self.limit == 0 {
            return;
        }
        if self.heap.len() < self.limit {
            self.heap.push(Candidate {
                score,
                seq: self.next_seq,
                id: id.to_string(),
            });
        } else if self.heap.peek().is_some_and(|worst| score > worst.score) {
            // Evict the current worst (lowest score); ties keep earlier arrivals.
            let _ = self.heap.pop();
            self.heap.push(Candidate {
                score,
                seq: self.next_seq,
                id: id.to_string(),
            });
        } else {
            // Did not make the cut: no id allocation for this row.
            return;
        }
        self.next_seq = self.next_seq.wrapping_add(1);
    }

    /// The kept candidates, sorted from highest score to lowest, ties in
    /// arrival order. This is the same ranking the sorted-buffer approach gave.
    pub fn into_sorted(self) -> Vec<(f32, String)> {
        let mut items: Vec<Candidate> = self.heap.into_iter().collect();
        items.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.seq.cmp(&b.seq))
        });
        items.into_iter().map(|c| (c.score, c.id)).collect()
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
    use super::TopK;

    #[test]
    fn top_k_holds_the_best_scores_in_order() {
        let mut best = TopK::new(3);
        let scored = [
            (0.2, "a"),
            (0.9, "b"),
            (f32::NAN, "not a number"),
            (0.5, "c"),
            (0.1, "d"),
            (0.7, "e"),
        ];
        for (score, id) in scored {
            best.offer(score, id);
        }
        let sorted = best.into_sorted();
        let ids: Vec<&str> = sorted.iter().map(|(_, id)| id.as_str()).collect();
        assert_eq!(ids, ["b", "e", "c"]);

        let mut none = TopK::new(0);
        none.offer(1.0, "x");
        assert!(none.into_sorted().is_empty());
    }
}
