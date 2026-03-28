//! Memory retrieval system
//! 
//! Implements memory retrieval with semantic search as specified in SPEC.md

use crate::models::QueryResult;
use crate::memory::embedding::EmbeddingClient;
use std::sync::Arc;

/// Memory retriever for finding relevant memories
#[allow(dead_code)]
pub struct MemoryRetriever {
    embedding_client: Arc<EmbeddingClient>,
    #[allow(dead_code)]
    default_namespace: String,
    #[allow(dead_code)]
    min_similarity: f32,
    #[allow(dead_code)]
    query_limit: usize,
}

impl MemoryRetriever {
    /// Create a new memory retriever
    #[allow(dead_code)]
    pub fn new(
        embedding_client: Arc<EmbeddingClient>,
        default_namespace: String,
        min_similarity: f32,
        query_limit: usize,
    ) -> Self {
        Self {
            embedding_client,
            default_namespace,
            min_similarity,
            query_limit,
        }
    }

    /// Get context string from retrieved memories
    pub fn get_context_string(&self, results: &[QueryResult]) -> String {
        results
            .iter()
            .map(|r| format!("[{}] {}", r.memory.tags.join(", "), r.memory.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get the default namespace
    #[allow(dead_code)]
    pub fn default_namespace(&self) -> &str {
        &self.default_namespace
    }

    /// Get minimum similarity threshold
    #[allow(dead_code)]
    pub fn min_similarity(&self) -> f32 {
        self.min_similarity
    }

    /// Get query limit
    #[allow(dead_code)]
    pub fn query_limit(&self) -> usize {
        self.query_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Memory, MemoryType};

    #[test]
    fn test_retriever_context_string() {
        let config = crate::memory::embedding::EmbeddingClientConfig::default();
        let embedding_client = Arc::new(EmbeddingClient::new(config));
        
        let retriever = MemoryRetriever::new(
            embedding_client,
            "retrieval".to_string(),
            0.5,
            10,
        );

        let memory1 = Memory::with_params(
            "Rust is a systems programming language".to_string(),
            "retrieval".to_string(),
            vec!["rust".to_string(), "programming".to_string()],
            Some(MemoryType::Concept),
            false,
        );

        let memory2 = Memory::with_params(
            "The user prefers dark mode".to_string(),
            "retrieval".to_string(),
            vec!["preference".to_string()],
            Some(MemoryType::Fact),
            false,
        );

        let results = vec![
            QueryResult { memory: memory1, score: 0.9 },
            QueryResult { memory: memory2, score: 0.7 },
        ];

        let context = retriever.get_context_string(&results);
        
        assert!(context.contains("rust"));
        assert!(context.contains("preference"));
        assert!(context.contains("dark mode"));
    }

    #[test]
    fn test_retriever_empty_results() {
        let config = crate::memory::embedding::EmbeddingClientConfig::default();
        let embedding_client = Arc::new(EmbeddingClient::new(config));
        
        let retriever = MemoryRetriever::new(
            embedding_client,
            "retrieval".to_string(),
            0.5,
            10,
        );

        let context = retriever.get_context_string(&[]);
        assert!(context.is_empty());
    }

    #[test]
    fn test_retriever_properties() {
        let config = crate::memory::embedding::EmbeddingClientConfig::default();
        let embedding_client = Arc::new(EmbeddingClient::new(config));
        
        let retriever = MemoryRetriever::new(
            embedding_client,
            "training".to_string(),
            0.7,
            20,
        );

        assert_eq!(retriever.default_namespace(), "training");
        assert!((retriever.min_similarity() - 0.7).abs() < 0.001);
        assert_eq!(retriever.query_limit(), 20);
    }
}
