//! Search Learning Service
//! 
//! Implements search learning as specified in SPEC.md:
//! - Detect knowledge gaps from sessions
//! - Search for relevant information using SearXNG
//! - Fetch and summarize content
//! - Extract key concepts and add to training memory

use crate::models::{Memory, MemoryType};
use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for search learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchLearningConfig {
    /// Enable search learning service
    pub enabled: bool,
    /// SearXNG instance URL
    pub searx_url: String,
    /// Timeout for search requests in seconds
    pub timeout_seconds: u64,
    /// Maximum results per search
    pub max_results: usize,
    /// Whether to fetch page content
    pub fetch_content: bool,
    /// Whether to generate summaries
    pub generate_summaries: bool,
    /// Topics to auto-research
    pub auto_research_topics: Vec<String>,
}

impl Default for SearchLearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            searx_url: "http://localhost:8088".to_string(),
            timeout_seconds: 30,
            max_results: 5,
            fetch_content: true,
            generate_summaries: true,
            auto_research_topics: Vec::new(),
        }
    }
}

/// A research topic to investigate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchTopic {
    pub id: String,
    pub topic: String,
    pub reason: String,
    pub priority: f32,
    pub created_at: String,
    pub status: ResearchStatus,
    pub results: Vec<SearchResult>,
}

impl ResearchTopic {
    pub fn new(topic: String, reason: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            topic,
            reason,
            priority: 0.5,
            created_at: Utc::now().to_rfc3339(),
            status: ResearchStatus::Pending,
            results: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_priority(mut self, priority: f32) -> Self {
        self.priority = priority;
        self
    }
}

/// Research status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResearchStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl Default for ResearchStatus {
    fn default() -> Self {
        ResearchStatus::Pending
    }
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub content: Option<String>,
    pub summary: Option<String>,
}

/// Service statistics
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchLearningStats {
    pub topics_researched: usize,
    pub searches_performed: usize,
    pub pages_fetched: usize,
    pub concepts_learned: usize,
    pub errors: usize,
    pub last_research: Option<String>,
}

/// Search learning service
#[derive(Clone)]
pub struct SearchLearningService {
    config: SearchLearningConfig,
    client: Client,
    stats: Arc<RwLock<SearchLearningStats>>,
    pending_topics: Arc<RwLock<Vec<ResearchTopic>>>,
}

impl SearchLearningService {
    pub fn new() -> Self {
        Self::with_config(SearchLearningConfig::default())
    }

    pub fn with_config(config: SearchLearningConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .unwrap_or_default();

        Self {
            config,
            client,
            stats: Arc::new(RwLock::new(SearchLearningStats::default())),
            pending_topics: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> SearchLearningStats {
        self.stats.read().await.clone()
    }

    /// Add a topic to research
    pub async fn add_topic(&self, topic: ResearchTopic) {
        let mut topics = self.pending_topics.write().await;
        if !topics.iter().any(|t| t.topic == topic.topic) {
            topics.push(topic);
        }
    }

    /// Add a topic from a knowledge gap
    pub async fn add_knowledge_gap(&self, gap: &str) {
        let topic = ResearchTopic::new(
            gap.to_string(),
            "Knowledge gap detected in conversation".to_string(),
        );
        self.add_topic(topic).await;
    }

    /// Get pending topics (for future use)
    #[allow(dead_code)]
    pub async fn get_pending_topics(&self) -> Vec<ResearchTopic> {
        self.pending_topics.read().await.clone()
    }

    /// Perform a web search using SearXNG
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let mut stats = self.stats.write().await;
        stats.searches_performed += 1;

        let url = format!("{}/search", self.config.searx_url);
        
        let response = self.client
            .get(&url)
            .query(&[
                ("q", query),
                ("format", "json"),
                ("engines", "google,duckduckgo,bing"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            stats.errors += 1;
            anyhow::bail!("Search failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct SearxResponse {
            results: Vec<SearxResult>,
        }

        #[derive(Deserialize)]
        struct SearxResult {
            title: String,
            url: String,
            content: Option<String>,
        }

        let searx_response: SearxResponse = response.json().await?;

        let results: Vec<SearchResult> = searx_response.results
            .into_iter()
            .take(self.config.max_results)
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.unwrap_or_default(),
                content: None,
                summary: None,
            })
            .collect();

        Ok(results)
    }

    /// Fetch page content
    pub async fn fetch_page(&self, url: &str) -> Result<String> {
        let mut stats = self.stats.write().await;
        stats.pages_fetched += 1;

        let response = self.client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; AGI-Agent/1.0)")
            .send()
            .await?;

        if !response.status().is_success() {
            stats.errors += 1;
            anyhow::bail!("Failed to fetch page: {}", response.status());
        }

        let content = response.text().await?;
        
        // Simple HTML tag stripping
        let text = strip_html(&content);
        
        Ok(text)
    }

    /// Research a topic end-to-end
    pub async fn research_topic(&self, topic: &mut ResearchTopic) -> Result<Vec<SearchResult>> {
        topic.status = ResearchStatus::InProgress;
        
        // Perform search
        let results = self.search(&topic.topic).await?;
        
        // Optionally fetch content from top results
        if self.config.fetch_content {
            let mut enriched_results = Vec::new();
            
            for result in results.into_iter().take(3) {
                let mut enriched = result;
                
                // Try to fetch content
                if let Ok(content) = self.fetch_page(&enriched.url).await {
                    enriched.content = Some(content.chars().take(2000).collect());
                    
                    // Generate summary if enabled
                    if self.config.generate_summaries {
                        enriched.summary = Some(self.generate_summary(&enriched.content.clone().unwrap_or_default()));
                    }
                }
                
                enriched_results.push(enriched);
            }
            
            topic.results = enriched_results.clone();
            topic.status = ResearchStatus::Completed;
            
            Ok(enriched_results)
        } else {
            topic.results = results.clone();
            topic.status = ResearchStatus::Completed;
            Ok(results)
        }
    }

    /// Generate a simple summary from content
    fn generate_summary(&self, content: &str) -> String {
        // Simple extractive summarization
        let sentences: Vec<&str> = content
            .split(|c| c == '.' || c == '!' || c == '?')
            .filter(|s| s.trim().len() > 20)
            .take(3)
            .collect();
        
        sentences.join(". ").trim().to_string()
    }

    /// Extract concepts from search results and create memories
    pub async fn extract_concepts(&self, results: &[SearchResult]) -> Vec<Memory> {
        let mut memories = Vec::new();

        for result in results {
            // Create memory from search result
            let mut memory = Memory::new(
                format!("{}: {}", result.title, result.snippet),
                "training".to_string(),
            );
            memory.memory_type = MemoryType::Concept;
            memory.tags = vec!["learned".to_string(), "search".to_string()];
            memory.evict_to_training = false;
            memory.is_persistent = true;
            
            // Store source URL in metadata
            memory.metadata.insert(
                "source_url".to_string(),
                serde_json::json!(result.url),
            );
            
            memories.push(memory);

            // Also create memory from full content if available
            if let Some(content) = &result.content {
                let mut content_memory = Memory::new(
                    content.chars().take(500).collect::<String>(),
                    "training".to_string(),
                );
                content_memory.memory_type = MemoryType::Concept;
                content_memory.tags = vec!["learned".to_string(), "detailed".to_string()];
                content_memory.is_persistent = true;
                memories.push(content_memory);
            }
        }

        let mut stats = self.stats.write().await;
        stats.concepts_learned += memories.len();

        memories
    }

    /// Process pending topics
    pub async fn process_pending(&self) -> Vec<ResearchTopic> {
        let mut topics = self.pending_topics.write().await;
        let mut processed = Vec::new();

        for topic in topics.iter_mut() {
            if topic.status == ResearchStatus::Pending {
                match self.research_topic(topic).await {
                    Ok(_) => {
                        processed.push(topic.clone());
                    }
                    Err(e) => {
                        tracing::warn!("Failed to research topic '{}': {}", topic.topic, e);
                        topic.status = ResearchStatus::Failed;
                    }
                }
            }
        }

        // Remove processed topics
        topics.retain(|t| t.status == ResearchStatus::Pending);

        let mut stats = self.stats.write().await;
        stats.topics_researched += processed.len();
        stats.last_research = Some(Utc::now().to_rfc3339());

        processed
    }

    /// Learn from a topic and generate memories
    pub async fn learn_from_topic(&self, topic: &ResearchTopic) -> Vec<Memory> {
        self.extract_concepts(&topic.results).await
    }
}

impl Default for SearchLearningService {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip HTML tags from content
fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        
        if c == '<' {
            in_tag = true;
            
            // Check for script/style tags
            let remaining = &html[i..];
            let lower = remaining.to_lowercase();
            
            if lower.starts_with("<script") {
                in_script = true;
            } else if lower.starts_with("<style") {
                in_style = true;
            } else if lower.starts_with("</script") {
                in_script = false;
            } else if lower.starts_with("</style") {
                in_style = false;
            }
        } else if c == '>' {
            in_tag = false;
            if !in_script && !in_style {
                result.push(' ');
            }
        } else if !in_tag && !in_script && !in_style {
            result.push(c);
        }
        
        i += 1;
    }

    // Clean up whitespace
    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        let html = "<p>Hello <b>world</b>!</p><script>alert('hi')</script>";
        let text = strip_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("<script>"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn test_research_topic_creation() {
        let topic = ResearchTopic::new(
            "Rust programming".to_string(),
            "User asked about Rust".to_string(),
        );
        
        assert!(!topic.id.is_empty());
        assert_eq!(topic.topic, "Rust programming");
        assert_eq!(topic.status, ResearchStatus::Pending);
        assert_eq!(topic.priority, 0.5);
    }

    #[test]
    fn test_search_learning_stats() {
        let stats = SearchLearningStats::default();
        assert_eq!(stats.topics_researched, 0);
        assert_eq!(stats.searches_performed, 0);
        assert_eq!(stats.concepts_learned, 0);
    }

    #[test]
    fn test_search_learning_service_creation() {
        let service = SearchLearningService::new();
        assert!(service.config.enabled);
        assert_eq!(service.config.searx_url, "http://localhost:8088");
    }

    #[test]
    fn test_add_topic() {
        let service = SearchLearningService::new();
        
        let topic = ResearchTopic::new(
            "Test topic".to_string(),
            "Test reason".to_string(),
        );
        
        tokio_test::block_on(service.add_topic(topic));
        
        let topics = tokio_test::block_on(service.get_pending_topics());
        assert_eq!(topics.len(), 1);
    }

    #[test]
    fn test_add_knowledge_gap() {
        let service = SearchLearningService::new();
        
        tokio_test::block_on(service.add_knowledge_gap("How does neural network work?"));
        
        let topics = tokio_test::block_on(service.get_pending_topics());
        assert_eq!(topics.len(), 1);
        assert!(topics[0].topic.contains("neural network"));
    }

    #[test]
    fn test_no_duplicate_topics() {
        let service = SearchLearningService::new();
        
        let topic1 = ResearchTopic::new("Same topic".to_string(), "Reason 1".to_string());
        let topic2 = ResearchTopic::new("Same topic".to_string(), "Reason 2".to_string());
        
        tokio_test::block_on(service.add_topic(topic1));
        tokio_test::block_on(service.add_topic(topic2));
        
        let topics = tokio_test::block_on(service.get_pending_topics());
        assert_eq!(topics.len(), 1);
    }

    #[test]
    fn test_extract_concepts() {
        let service = SearchLearningService::new();
        
        let results = vec![
            SearchResult {
                title: "Rust Programming".to_string(),
                url: "https://rust.example.com".to_string(),
                snippet: "Rust is a systems programming language.".to_string(),
                content: Some("Rust focuses on safety and performance.".to_string()),
                summary: None,
            },
        ];
        
        let memories = tokio_test::block_on(service.extract_concepts(&results));
        
        assert_eq!(memories.len(), 2); // One from snippet, one from content
        assert_eq!(memories[0].namespace, "training");
        assert_eq!(memories[0].memory_type, MemoryType::Concept);
    }
}
