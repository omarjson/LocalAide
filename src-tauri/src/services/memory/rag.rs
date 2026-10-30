use std::sync::Arc;
use crate::app::error::AppError;
use crate::domain::memory::Citation;
use crate::services::library::search::SearchMode;
use crate::services::library::LibraryService;

pub const MIN_RELEVANCE_SCORE: f32 = 0.010; // Calibrated relevance gate threshold for RRF scores
pub const DEFAULT_RAG_BUDGET: usize = 1024; // Maximum tokens reserved for RAG context

pub struct RagRecallResult {
    pub citations: Vec<Citation>,
    pub context_prompt: String,
    pub total_tokens: usize,
}

pub struct RagEngine {
    library: Arc<LibraryService>,
}

impl RagEngine {
    pub fn new(library: Arc<LibraryService>) -> Self {
        Self { library }
    }

    pub fn recall(
        &self,
        query: &str,
        max_tokens: Option<usize>,
    ) -> Result<RagRecallResult, AppError> {
        let budget = max_tokens.unwrap_or(DEFAULT_RAG_BUDGET);
        if query.trim().is_empty() {
            return Ok(RagRecallResult {
                citations: Vec::new(),
                context_prompt: String::new(),
                total_tokens: 0,
            });
        }

        // Query library hybrid search
        let raw_hits = self.library.search(query, SearchMode::Hybrid, 10)?;

        let mut citations = Vec::new();
        let mut context_chunks = Vec::new();
        let mut accumulated_tokens = 0;

        for hit in raw_hits {
            // Relevance gate: filter out low-confidence hits
            if hit.score < MIN_RELEVANCE_SCORE {
                continue;
            }

            let chunk_tokens = hit.snippet.split_whitespace().count();
            if accumulated_tokens + chunk_tokens > budget {
                // Respect token budget strictly
                break;
            }

            let citation_index = citations.len() + 1;
            citations.push(Citation {
                index: citation_index,
                source: hit.path.clone(),
                title: hit.title.clone(),
                snippet: hit.snippet.clone(),
                score: hit.score,
            });

            context_chunks.push(format!(
                "[{}] (Source: {}):\n{}",
                citation_index, hit.path, hit.snippet
            ));

            accumulated_tokens += chunk_tokens;
        }

        if citations.is_empty() {
            return Ok(RagRecallResult {
                citations: Vec::new(),
                context_prompt: String::new(),
                total_tokens: 0,
            });
        }

        let context_prompt = format!(
            "\nContext from knowledge library:\n{}\nInstructions for answering:\n- Answer the user's inquiry based on the context above when relevant.\n- You MUST cite the source keys (e.g. [1], [2]) for every factual claim derived from this context.\n- If the context does not answer the question, state that clearly.\n",
            context_chunks.join("\n\n")
        );

        Ok(RagRecallResult {
            citations,
            context_prompt,
            total_tokens: accumulated_tokens,
        })
    }
}
