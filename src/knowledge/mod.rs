//! External Knowledge Base Integration
//!
//! Provides integration with external knowledge sources:
//! - Wikipedia API
//! - ArXiv API
//! - Web fetching with content extraction
//! - Custom knowledge endpoints

mod wikipedia;
mod arxiv;
mod fetch;

pub use wikipedia::WikipediaClient;
pub use arxiv::ArxivClient;
pub use fetch::WebFetcher;

/// Knowledge source types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeSource {
    Wikipedia,
    Arxiv,
    Web,
    Custom,
}

/// A piece of knowledge from an external source
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeEntry {
    pub source: KnowledgeSource,
    pub title: String,
    pub content: String,
    pub url: Option<String>,
    pub relevance_score: f32,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl KnowledgeEntry {
    pub fn new(source: KnowledgeSource, title: String, content: String) -> Self {
        Self {
            source,
            title,
            content,
            url: None,
            relevance_score: 1.0,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_relevance(mut self, score: f32) -> Self {
        self.relevance_score = score;
        self
    }

    pub fn add_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
    }

    /// Convert to memory format for storage (for future use)
    #[allow(dead_code)]
    pub fn to_memory(&self, namespace: &str) -> crate::models::Memory {
        let mut memory = crate::models::Memory::new(
            format!("{}: {}", self.title, self.content),
            namespace.to_string(),
        );
        memory.tags = vec![
            format!("source:{:?}", self.source).to_lowercase(),
            "external".to_string(),
        ];
        if let Some(url) = &self.url {
            memory.metadata.insert("source_url".to_string(), serde_json::json!(url));
        }
        memory
    }
}

/// Configuration for external knowledge sources
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeConfig {
    pub enabled: bool,
    pub wikipedia_enabled: bool,
    pub arxiv_enabled: bool,
    pub web_fetch_enabled: bool,
    pub fetch_timeout_seconds: u64,
    pub max_content_length: usize,
    pub min_relevance_score: f32,
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            wikipedia_enabled: true,
            arxiv_enabled: true,
            web_fetch_enabled: true,
            fetch_timeout_seconds: 30,
            max_content_length: 10000,
            min_relevance_score: 0.5,
        }
    }
}
