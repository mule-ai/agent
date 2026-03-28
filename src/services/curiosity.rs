//! Curiosity-Driven Exploration Service
//! 
//! Implements Phase 3 "Curiosity-driven exploration" as specified in SPEC.md:
//! - Detects knowledge gaps from conversations and responses
//! - Tracks areas of uncertainty
//! - Triggers autonomous exploration and learning
//! - Adapts exploration based on curiosity scores
//! - Integrates with search learning for actual research

use crate::models::{Memory, MemoryType};
use crate::services::search_learning::SearchLearningService;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for curiosity-driven exploration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuriosityConfig {
    /// Enable curiosity-driven exploration
    pub enabled: bool,
    /// Minimum curiosity score to trigger exploration
    pub curiosity_threshold: f32,
    /// Maximum explorations per session
    pub max_explorations_per_session: usize,
    /// Maximum concurrent explorations
    pub max_concurrent_explorations: usize,
    /// How often to check for exploration opportunities (in seconds)
    pub check_interval_seconds: u64,
    /// Enable deep exploration (follow links, gather multiple sources)
    pub deep_exploration: bool,
    /// Confidence threshold for uncertainty detection
    pub uncertainty_threshold: f32,
    /// Topics to always explore
    pub always_explore_topics: Vec<String>,
}

impl Default for CuriosityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            curiosity_threshold: 0.5,
            max_explorations_per_session: 5,
            max_concurrent_explorations: 3,
            check_interval_seconds: 300, // 5 minutes
            deep_exploration: true,
            uncertainty_threshold: 0.4,
            always_explore_topics: vec![
                "artificial intelligence".to_string(),
                "machine learning".to_string(),
                "programming".to_string(),
            ],
        }
    }
}

/// Knowledge gap detected by the curiosity engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub id: String,
    /// The topic or concept that needs exploration
    pub topic: String,
    /// Why this is a knowledge gap
    pub reason: KnowledgeGapReason,
    /// How curious we are about this topic (0.0 - 1.0)
    pub curiosity_score: f32,
    /// Confidence level about the topic (0.0 - 1.0)
    pub confidence: f32,
    /// When this gap was detected
    pub detected_at: String,
    /// Current status
    pub status: GapStatus,
    /// Exploration results
    pub exploration: Option<ExplorationResult>,
    /// Keywords related to this topic
    pub related_topics: Vec<String>,
}

impl KnowledgeGap {
    pub fn new(topic: String, reason: KnowledgeGapReason) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            topic,
            reason,
            curiosity_score: 0.5,
            confidence: 0.5,
            detected_at: Utc::now().to_rfc3339(),
            status: GapStatus::Detected,
            exploration: None,
            related_topics: Vec::new(),
        }
    }

    /// Calculate curiosity score based on gap characteristics
    pub fn calculate_curiosity(&self) -> f32 {
        let base = self.confidence.max(0.0).min(1.0);
        let uncertainty = 1.0 - base;
        
        // Boost score based on importance of reason
        let reason_boost = match &self.reason {
            KnowledgeGapReason::UserQuestion { .. } => 0.2,
            KnowledgeGapReason::AgentUncertainty { .. } => 0.3,
            KnowledgeGapReason::FailedSearch { .. } => 0.25,
            KnowledgeGapReason::Contradiction { .. } => 0.4,
            KnowledgeGapReason::MissingContext => 0.15,
            KnowledgeGapReason::TopicMention { .. } => 0.1,
            KnowledgeGapReason::NovelConcept { .. } => 0.35,
        };
        
        (uncertainty * 0.7 + reason_boost * 0.3).min(1.0)
    }
}

/// Reason for detecting a knowledge gap
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnowledgeGapReason {
    /// Agent was asked a question it couldn't answer well
    UserQuestion { question: String },
    /// Agent expressed uncertainty
    AgentUncertainty { statement: String },
    /// A search query failed to return useful results
    FailedSearch { query: String },
    /// Contradiction detected in knowledge
    Contradiction { statement: String },
    /// Context needed but not available
    MissingContext,
    /// Topic was mentioned but not well understood
    TopicMention { topic: String },
    /// New/unknown concept encountered
    NovelConcept { concept: String },
}

/// Status of a knowledge gap
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GapStatus {
    Detected,
    Queued,
    Exploring,
    Explored,
    Dismissed,
}

impl Default for GapStatus {
    fn default() -> Self {
        GapStatus::Detected
    }
}

/// Result of exploring a knowledge gap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationResult {
    /// Summary of what was learned
    pub summary: String,
    /// Key facts discovered
    pub facts: Vec<String>,
    /// Concepts understood
    pub concepts: Vec<String>,
    /// Sources consulted
    pub sources: Vec<String>,
    /// Depth of exploration (shallow, moderate, deep)
    pub depth: ExplorationDepth,
    /// Time spent exploring in seconds
    pub duration_seconds: u64,
    /// Memory IDs created from exploration
    pub memory_ids: Vec<String>,
}

impl ExplorationResult {
    pub fn new() -> Self {
        Self {
            summary: String::new(),
            facts: Vec::new(),
            concepts: Vec::new(),
            sources: Vec::new(),
            depth: ExplorationDepth::Moderate,
            duration_seconds: 0,
            memory_ids: Vec::new(),
        }
    }
}

impl Default for ExplorationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Depth of exploration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExplorationDepth {
    Shallow,
    Moderate,
    Deep,
}

impl Default for ExplorationDepth {
    fn default() -> Self {
        ExplorationDepth::Moderate
    }
}

/// An exploration task in the queue
#[derive(Debug, Clone)]
pub struct ExplorationTask {
    pub gap_id: String,
    pub topic: String,
    #[allow(dead_code)]
    pub depth: ExplorationDepth,
    pub priority: f32,
    #[allow(dead_code)]
    pub created_at: String,
}

impl ExplorationTask {
    pub fn from_gap(gap: &KnowledgeGap) -> Self {
        Self {
            gap_id: gap.id.clone(),
            topic: gap.topic.clone(),
            depth: ExplorationDepth::Moderate,
            priority: gap.curiosity_score,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Service statistics
#[derive(Debug, Clone, Default, Serialize)]
pub struct CuriosityStats {
    pub gaps_detected: usize,
    pub gaps_explored: usize,
    pub gaps_dismissed: usize,
    pub explorations_performed: usize,
    pub concepts_learned: usize,
    pub avg_curiosity_score: f32,
    pub errors: usize,
    pub last_activity: Option<String>,
}

/// Curiosity engine for detecting and exploring knowledge gaps
#[derive(Clone)]
pub struct CuriosityEngine {
    config: CuriosityConfig,
    knowledge_client: KnowledgeClient,
    search_service: SearchLearningService,
    /// Known gaps
    gaps: Arc<RwLock<HashMap<String, KnowledgeGap>>>,
    /// Exploration queue
    queue: Arc<RwLock<Vec<ExplorationTask>>>,
    /// Statistics
    stats: Arc<RwLock<CuriosityStats>>,
    /// Topic interest scores (builds over time)
    topic_interests: Arc<RwLock<HashMap<String, f32>>>,
}

impl CuriosityEngine {
    /// Create a new curiosity engine with default config
    pub fn new() -> Self {
        Self::with_config(CuriosityConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: CuriosityConfig) -> Self {
        let search_service = SearchLearningService::with_config(
            crate::services::search_learning::SearchLearningConfig::default()
        );
        
        Self {
            config,
            knowledge_client: KnowledgeClient::default(),
            search_service,
            gaps: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(CuriosityStats::default())),
            topic_interests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> CuriosityStats {
        self.stats.read().await.clone()
    }

    /// Wire the internal search learning service to batch training
    /// This enables curiosity-driven exploration to generate training examples
    pub async fn wire_to_batch_training(&self, batch_service: crate::services::BatchTrainingService) {
        self.search_service.set_batch_training_service(batch_service).await;
        tracing::debug!("Curiosity engine wired to batch training service");
    }

    /// Get the internal search learning service for wiring (advanced use)
    #[allow(dead_code)]
    pub fn get_search_service(&self) -> SearchLearningService {
        self.search_service.clone()
    }

    /// Detect knowledge gaps from a conversation
    pub async fn detect_gaps(&self, messages: &[crate::models::Message]) -> Vec<KnowledgeGap> {
        let mut detected = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            // Check for user questions the agent might struggle with
            if msg.role == crate::models::Role::User {
                let question = &msg.content;
                
                // Simple heuristics for gap detection
                // 1. Questions with specific facts
                if self.looks_like_specific_fact_question(question) {
                    let gap = KnowledgeGap::new(
                        self.extract_topic(question),
                        KnowledgeGapReason::UserQuestion { question: question.clone() },
                    );
                    let gap = self.assess_gap(gap, question);
                    detected.push(gap);
                }
                
                // 2. Questions about recent topics
                if question.to_lowercase().contains("how") 
                    || question.to_lowercase().contains("why")
                    || question.to_lowercase().contains("what")
                {
                    let gap = KnowledgeGap::new(
                        self.extract_topic(question),
                        KnowledgeGapReason::TopicMention {
                            topic: self.extract_topic(question),
                        },
                    );
                    let gap = self.assess_gap(gap, question);
                    detected.push(gap);
                }
            }
            
            // Check for agent uncertainty in previous response
            if msg.role == crate::models::Role::Assistant {
                if self.agent_expressed_uncertainty(&msg.content) {
                    let gap = KnowledgeGap::new(
                        self.extract_topic(&msg.content),
                        KnowledgeGapReason::AgentUncertainty {
                            statement: self.extract_uncertain_statement(&msg.content),
                        },
                    );
                    let gap = self.assess_gap(gap, &msg.content);
                    detected.push(gap);
                }
            }
            
            // Check for novel concepts
            if i > 0 && msg.role == crate::models::Role::User {
                let novel = self.detect_novel_concepts(&msg.content, messages.get(i.saturating_sub(1)));
                for concept in novel {
                    let mut gap = KnowledgeGap::new(
                        concept.clone(),
                        KnowledgeGapReason::NovelConcept { concept },
                    );
                    gap.curiosity_score = self.calculate_curiosity(&gap);
                    detected.push(gap);
                }
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.gaps_detected += detected.len();
        }

        // Add to gaps map and queue interesting ones
        let mut gaps_to_queue = Vec::new();
        for mut gap in detected {
            gap.curiosity_score = gap.calculate_curiosity();
            
            // Store gap
            let mut gaps = self.gaps.write().await;
            gaps.insert(gap.id.clone(), gap.clone());
            
            // Queue if above threshold
            if gap.curiosity_score >= self.config.curiosity_threshold {
                gap.status = GapStatus::Queued;
                gaps_to_queue.push(ExplorationTask::from_gap(&gap));
            }
        }

        // Add to queue
        if !gaps_to_queue.is_empty() {
            let mut queue = self.queue.write().await;
            for task in gaps_to_queue {
                if !queue.iter().any(|t| t.topic == task.topic) {
                    queue.push(task);
                }
            }
            // Sort by priority
            queue.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
        }

        self.gaps.read().await.values().cloned().collect()
    }

    /// Assess a gap and estimate curiosity score
    fn assess_gap(&self, mut gap: KnowledgeGap, context: &str) -> KnowledgeGap {
        // Estimate confidence based on context
        gap.confidence = self.estimate_confidence(context);
        gap.curiosity_score = gap.calculate_curiosity();
        gap
    }

    /// Check if a question asks for specific facts
    fn looks_like_specific_fact_question(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        
        // Questions about specific things
        let specific_patterns = [
            "when was", "when did", "when does",
            "where is", "where did", "where was",
            "who invented", "who discovered", "who created",
            "what year", "what date", "what percentage",
            "how many", "how much", "how long ago",
        ];
        
        specific_patterns.iter().any(|p| lower.contains(p))
    }

    /// Extract the main topic from a question
    fn extract_topic(&self, text: &str) -> String {
        let lower = text.to_lowercase();
        
        // Remove question words (order matters: longer phrases first to avoid partial matches)
        let cleaned = lower
            .replace("what is", "")
            .replace("what are", "")
            .replace("what was", "")
            .replace("what does", "")
            .replace("how does", "")
            .replace("how did", "")
            .replace("why is", "")
            .replace("who was", "")
            .replace("where was", "")
            .replace("how do", "")
            .replace("why do", "")
            .replace("who is", "")
            .replace("where is", "")
            .replace("?", "")
            .trim()
            .to_string();
        
        // Take first meaningful phrase
        cleaned
            .split(&[' ', ',', '.'][..])
            .take(5)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Check if agent expressed uncertainty
    fn agent_expressed_uncertainty(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        
        let uncertainty_phrases = [
            "i'm not sure",
            "i don't know",
            "i'm uncertain",
            "i cannot be certain",
            "it might be",
            "possibly",
            "perhaps",
            "maybe",
            "i'm not certain",
            "i don't have information",
            "i'm not certain about",
            "without more information",
            "i'm not sure if",
            "i'm not familiar with",
        ];
        
        uncertainty_phrases.iter().any(|phrase| lower.contains(phrase))
    }

    /// Extract the uncertain statement
    fn extract_uncertain_statement(&self, text: &str) -> String {
        let lower = text.to_lowercase();
        
        let phrases = [
            "i'm not sure",
            "i don't know",
            "i'm uncertain",
            "i cannot be certain",
            "it might be",
        ];
        
        for phrase in phrases {
            if let Some(pos) = lower.find(phrase) {
                let start = pos.saturating_sub(20);
                let end = (pos + 100).min(text.len());
                return text[start..end].trim().to_string();
            }
        }
        
        text.chars().take(100).collect()
    }

    /// Detect novel concepts in the text
    fn detect_novel_concepts(&self, text: &str, _previous: Option<&crate::models::Message>) -> Vec<String> {
        let lower = text.to_lowercase();
        let mut concepts = Vec::new();
        
        // Check for technical/scientific terms
        let tech_patterns = [
            "algorithm", "neural network", "quantum", "blockchain",
            "machine learning", "deep learning", "transformer",
            "cryptocurrency", "distributed", "encryption",
            "optimization", "encryption", "protocol",
        ];
        
        for pattern in tech_patterns {
            if lower.contains(pattern) && !self.has_topic_interest(pattern) {
                concepts.push(pattern.to_string());
            }
        }
        
        concepts
    }

    /// Check if we have interest in a topic
    fn has_topic_interest(&self, _topic: &str) -> bool {
        // For now, simple check
        // Could be enhanced with actual memory lookup
        false
    }

    /// Estimate confidence level based on context
    fn estimate_confidence(&self, _text: &str) -> f32 {
        // Simple heuristic: 50% base confidence
        // Could be enhanced with actual analysis
        0.5
    }

    /// Calculate curiosity score for a gap
    fn calculate_curiosity(&self, gap: &KnowledgeGap) -> f32 {
        let uncertainty = 1.0 - gap.confidence;
        let reason_weight = match &gap.reason {
            KnowledgeGapReason::UserQuestion { .. } => 0.8,
            KnowledgeGapReason::NovelConcept { .. } => 0.9,
            KnowledgeGapReason::Contradiction { .. } => 0.7,
            KnowledgeGapReason::FailedSearch { .. } => 0.6,
            KnowledgeGapReason::AgentUncertainty { .. } => 0.5,
            KnowledgeGapReason::TopicMention { .. } => 0.3,
            KnowledgeGapReason::MissingContext => 0.4,
        };
        
        (uncertainty * 0.6 + reason_weight * 0.4).min(1.0)
    }

    /// Get gaps that need exploration
    pub async fn get_pending_gaps(&self) -> Vec<KnowledgeGap> {
        self.gaps
            .read()
            .await
            .values()
            .filter(|g| g.status == GapStatus::Detected || g.status == GapStatus::Queued)
            .cloned()
            .collect()
    }

    /// Get all gaps
    pub async fn get_all_gaps(&self) -> Vec<KnowledgeGap> {
        self.gaps.read().await.values().cloned().collect()
    }

    /// Get exploration queue (for future use)
    #[allow(dead_code)]
    pub async fn get_queue(&self) -> Vec<ExplorationTask> {
        self.queue.read().await.clone()
    }

    /// Explore a knowledge gap
    pub async fn explore_gap(&self, gap_id: &str) -> Result<ExplorationResult> {
        let start_time = std::time::Instant::now();
        
        // Get gap from storage
        let gap = {
            let gaps = self.gaps.read().await;
            gaps.get(gap_id).cloned()
        };
        
        let gap = match gap {
            Some(g) => g,
            None => anyhow::bail!("Gap not found: {}", gap_id),
        };
        
        // Update status
        {
            let mut gaps = self.gaps.write().await;
            if let Some(g) = gaps.get_mut(gap_id) {
                g.status = GapStatus::Exploring;
            }
        }
        
        // Perform exploration
        let mut result = ExplorationResult::new();
        
        // 1. Search for information
        let search_results = self.search_service.search(&gap.topic).await.unwrap_or_default();
        
        for sr in &search_results {
            result.sources.push(sr.url.clone());
            
            // Extract facts
            if !sr.snippet.is_empty() {
                result.facts.push(sr.snippet.clone());
            }
        }
        
        // 2. Try Wikipedia if available
        if let Ok(wikipedia_content) = self.knowledge_client.fetch_wikipedia(&gap.topic).await {
            result.concepts.push(gap.topic.clone());
            if !result.summary.is_empty() {
                result.summary = format!("{} Wikipedia: {}", result.summary, &wikipedia_content[..wikipedia_content.len().min(200)]);
            } else {
                result.summary = wikipedia_content.chars().take(300).collect();
            }
            result.sources.push(format!("Wikipedia: {}", gap.topic));
        }
        
        // 3. Try ArXiv for academic topics
        let academic_terms = ["algorithm", "neural", "learning", "model", "network"];
        if academic_terms.iter().any(|t| gap.topic.to_lowercase().contains(t)) {
            if let Ok(arxiv_results) = self.knowledge_client.search_arxiv(&gap.topic).await {
                for paper in arxiv_results.iter().take(3) {
                    result.concepts.push(paper.title.clone());
                    result.sources.push(format!("ArXiv: {}", paper.id));
                }
            }
        }
        
        // 4. Set depth based on exploration
        result.depth = if self.config.deep_exploration && search_results.len() > 5 {
            ExplorationDepth::Deep
        } else if search_results.len() > 2 {
            ExplorationDepth::Moderate
        } else {
            ExplorationDepth::Shallow
        };
        
        result.duration_seconds = start_time.elapsed().as_secs();
        
        // Update gap with result
        {
            let mut gaps = self.gaps.write().await;
            if let Some(g) = gaps.get_mut(gap_id) {
                g.status = GapStatus::Explored;
                g.exploration = Some(result.clone());
                
                // Update confidence based on what we learned
                g.confidence = (g.confidence + 0.3).min(1.0);
            }
        }
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.gaps_explored += 1;
            stats.explorations_performed += 1;
            stats.concepts_learned += result.concepts.len();
            stats.last_activity = Some(Utc::now().to_rfc3339());
        }
        
        // Update topic interests
        {
            let mut interests = self.topic_interests.write().await;
            let score = interests.entry(gap.topic.clone()).or_insert(0.0);
            *score = (*score + gap.curiosity_score).min(1.0);
        }
        
        Ok(result)
    }

    /// Process the exploration queue
    pub async fn process_queue(&self) -> Vec<ExplorationResult> {
        let mut results = Vec::new();
        
        let queue_len = self.queue.read().await.len();
        let max_to_process = self.config.max_explorations_per_session.min(queue_len);
        
        for _ in 0..max_to_process {
            let task = {
                let mut queue = self.queue.write().await;
                queue.pop()
            };
            
            if let Some(task) = task {
                match self.explore_gap(&task.gap_id).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        tracing::warn!("Failed to explore gap {}: {}", task.gap_id, e);
                        let mut stats = self.stats.write().await;
                        stats.errors += 1;
                    }
                }
            }
        }
        
        results
    }

    /// Dismiss a gap (e.g., if it's not relevant)
    pub async fn dismiss_gap(&self, gap_id: &str) {
        let mut gaps = self.gaps.write().await;
        if let Some(g) = gaps.get_mut(gap_id) {
            g.status = GapStatus::Dismissed;
        }
        
        let mut stats = self.stats.write().await;
        stats.gaps_dismissed += 1;
        
        // Remove from queue
        let mut queue = self.queue.write().await;
        queue.retain(|t| t.gap_id != gap_id);
    }

    /// Convert exploration result to memories
    pub async fn result_to_memories(&self, result: &ExplorationResult, namespace: &str) -> Vec<Memory> {
        let mut memories = Vec::new();
        
        // Create summary memory
        if !result.summary.is_empty() {
            let mut memory = Memory::new(
                format!("Explored {}: {}", namespace, result.summary),
                namespace.to_string(),
            );
            memory.memory_type = MemoryType::Concept;
            memory.tags = vec!["curiosity".to_string(), "explored".to_string()];
            memory.metadata.insert("depth".to_string(), serde_json::json!(result.depth));
            memory.metadata.insert("sources".to_string(), serde_json::json!(result.sources));
            memories.push(memory);
        }
        
        // Create concept memories
        for concept in &result.concepts {
            let mut memory = Memory::new(
                concept.clone(),
                namespace.to_string(),
            );
            memory.memory_type = MemoryType::Concept;
            memory.tags = vec!["concept".to_string(), "learned".to_string(), "curiosity".to_string()];
            memory.is_persistent = true;
            memories.push(memory);
        }
        
        // Create fact memories
        for fact in &result.facts {
            let mut memory = Memory::new(
                fact.clone(),
                namespace.to_string(),
            );
            memory.memory_type = MemoryType::Fact;
            memory.tags = vec!["fact".to_string(), "learned".to_string()];
            memories.push(memory);
        }
        
        memories
    }

    /// Get topic interest scores (for future use)
    #[allow(dead_code)]
    pub async fn get_topic_interests(&self) -> HashMap<String, f32> {
        self.topic_interests.read().await.clone()
    }

    /// Check if a topic is worth exploring based on interests (for future use)
    #[allow(dead_code)]
    pub async fn is_interesting(&self, topic: &str) -> bool {
        let interests = self.topic_interests.read().await;
        
        // Check direct match
        if interests.contains_key(topic) {
            return true;
        }
        
        // Check if any interest is a substring
        for (interest, score) in interests.iter() {
            if topic.to_lowercase().contains(&interest.to_lowercase())
                || interest.to_lowercase().contains(&topic.to_lowercase())
            {
                if *score >= self.config.curiosity_threshold {
                    return true;
                }
            }
        }
        
        // Check always-explore topics
        if self.config.always_explore_topics.iter().any(|t| {
            topic.to_lowercase().contains(&t.to_lowercase())
        }) {
            return true;
        }
        
        false
    }
}

impl Default for CuriosityEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple knowledge client for curiosity exploration
#[derive(Debug, Clone, Default)]
pub struct KnowledgeClient {
    wikipedia_base: String,
    arxiv_base: String,
}

impl KnowledgeClient {
    pub fn new() -> Self {
        Self {
            wikipedia_base: "https://en.wikipedia.org/api/rest_v1/page/summary".to_string(),
            arxiv_base: "http://export.arxiv.org/api/query".to_string(),
        }
    }

    /// Fetch Wikipedia article summary
    pub async fn fetch_wikipedia(&self, topic: &str) -> Result<String> {
        let topic_encoded = topic.replace(' ', "_");
        let url = format!("{}/{}", self.wikipedia_base, topic_encoded);
        
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "AGI-Agent/1.0 (curiosity exploration)")
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Wikipedia fetch failed: {}", response.status());
        }
        
        #[derive(Deserialize)]
        struct WikiResponse {
            extract: Option<String>,
        }
        
        #[derive(Deserialize)]
        struct WikiError {
            #[serde(rename = "type")]
            error_type: Option<String>,
            title: Option<String>,
            #[allow(dead_code)]
            detail: Option<String>,
        }
        
        let text = response.text().await?;
        
        // Try to parse as success
        if let Ok(wiki_response) = serde_json::from_str::<WikiResponse>(&text) {
            if let Some(extract) = wiki_response.extract {
                return Ok(extract);
            }
        }
        
        // Check for error response
        if let Ok(error) = serde_json::from_str::<WikiError>(&text) {
            if error.error_type.is_some() {
                anyhow::bail!("Wikipedia error: {}", error.title.as_deref().unwrap_or("Unknown"));
            }
        }
        
        anyhow::bail!("Failed to parse Wikipedia response")
    }

    /// Search ArXiv for papers
    pub async fn search_arxiv(&self, query: &str) -> Result<Vec<ArxivPaper>> {
        // Use percent_encoding instead of urlencoding crate
        let encoded_query = query.replace(' ', "%20");
        let url = format!(
            "{}?search_query=all:{}&start=0&max_results=5",
            self.arxiv_base,
            encoded_query
        );
        
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "AGI-Agent/1.0 (curiosity exploration)")
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("ArXiv search failed: {}", response.status());
        }
        
        let text = response.text().await?;
        let papers = parse_arxiv_feed(&text)?;
        
        Ok(papers)
    }
}

/// ArXiv paper
#[derive(Debug, Clone, Default)]
pub struct ArxivPaper {
    pub id: String,
    pub title: String,
    #[allow(dead_code)]
    pub summary: String,
    pub authors: Vec<String>,
    #[allow(dead_code)]
    pub published: String,
}

/// Parse ArXiv Atom feed
fn parse_arxiv_feed(xml: &str) -> Result<Vec<ArxivPaper>> {
    let mut papers = Vec::new();
    
    // Simple XML parsing for ArXiv feed
    let entries: Vec<&str> = xml.split("<entry>").collect();
    
    for entry in entries.iter().skip(1) {
        let mut paper = ArxivPaper {
            id: extract_xml_tag(entry, "id").unwrap_or_default(),
            title: extract_xml_tag(entry, "title")
                .map(|s| s.replace('\n', " ").trim().to_string())
                .unwrap_or_default(),
            summary: extract_xml_tag(entry, "summary")
                .map(|s| s.replace('\n', " ").trim().to_string())
                .unwrap_or_default(),
            authors: Vec::new(),
            published: extract_xml_tag(entry, "published").unwrap_or_default(),
        };
        
        // Extract authors
        let author_section = entry.split("<author>").collect::<Vec<_>>();
        for author in author_section.iter().skip(1) {
            if let Some(name) = extract_xml_tag(author, "name") {
                paper.authors.push(name);
            }
        }
        
        if !paper.id.is_empty() || !paper.title.is_empty() {
            papers.push(paper);
        }
    }
    
    Ok(papers)
}

/// Extract content from an XML tag
fn extract_xml_tag(content: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    
    let start = content.find(&start_tag)? + start_tag.len();
    let end = content.find(&end_tag)?;
    
    Some(content[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curiosity_config_defaults() {
        let config = CuriosityConfig::default();
        assert!(config.enabled);
        assert_eq!(config.curiosity_threshold, 0.5);
        assert_eq!(config.max_explorations_per_session, 5);
    }

    #[test]
    fn test_knowledge_gap_creation() {
        let gap = KnowledgeGap::new(
            "Rust programming".to_string(),
            KnowledgeGapReason::TopicMention {
                topic: "Rust".to_string(),
            },
        );
        
        assert!(!gap.id.is_empty());
        assert_eq!(gap.topic, "Rust programming");
        assert_eq!(gap.status, GapStatus::Detected);
    }

    #[test]
    fn test_knowledge_gap_curiosity() {
        let mut gap = KnowledgeGap::new(
            "Quantum computing".to_string(),
            KnowledgeGapReason::NovelConcept {
                concept: "Quantum computing".to_string(),
            },
        );
        gap.confidence = 0.3;
        
        let curiosity = gap.calculate_curiosity();
        assert!(curiosity > 0.5); // Novel concepts should have high curiosity
    }

    #[test]
    fn test_gap_status() {
        assert_eq!(GapStatus::Detected, GapStatus::Detected);
        assert_eq!(GapStatus::Explored, GapStatus::Explored);
    }

    #[test]
    fn test_extract_topic() {
        let engine = CuriosityEngine::new();
        
        assert_eq!(
            engine.extract_topic("What is Rust programming?"),
            "rust programming"
        );
        assert_eq!(
            engine.extract_topic("How does neural network work?"),
            "neural network work"
        );
    }

    #[test]
    fn test_uncertainty_detection() {
        let engine = CuriosityEngine::new();
        
        assert!(engine.agent_expressed_uncertainty("I'm not sure about this."));
        assert!(engine.agent_expressed_uncertainty("I don't know the answer."));
        assert!(engine.agent_expressed_uncertainty("Maybe it's 42."));
        assert!(!engine.agent_expressed_uncertainty("The answer is 42."));
    }

    #[test]
    fn test_specific_fact_question() {
        let engine = CuriosityEngine::new();
        
        assert!(engine.looks_like_specific_fact_question("When was Rust invented?"));
        assert!(engine.looks_like_specific_fact_question("Who created Python?"));
        assert!(engine.looks_like_specific_fact_question("How many people use Linux?"));
        assert!(!engine.looks_like_specific_fact_question("Tell me about programming."));
    }

    #[test]
    fn test_exploration_task_from_gap() {
        let gap = KnowledgeGap::new(
            "Machine learning".to_string(),
            KnowledgeGapReason::UserQuestion {
                question: "What is machine learning?".to_string(),
            },
        );
        
        let task = ExplorationTask::from_gap(&gap);
        assert_eq!(task.gap_id, gap.id);
        assert_eq!(task.topic, "Machine learning");
    }

    #[test]
    fn test_curiosity_stats() {
        let stats = CuriosityStats::default();
        assert_eq!(stats.gaps_detected, 0);
        assert_eq!(stats.gaps_explored, 0);
        assert_eq!(stats.explorations_performed, 0);
    }

    #[tokio::test]
    async fn test_get_pending_gaps() {
        let engine = CuriosityEngine::new();
        let gaps = engine.get_pending_gaps().await;
        assert!(gaps.is_empty());
    }

    #[tokio::test]
    async fn test_topic_interests() {
        let engine = CuriosityEngine::new();
        
        let interests = engine.get_topic_interests().await;
        assert!(interests.is_empty());
    }

    #[tokio::test]
    async fn test_is_interesting() {
        let engine = CuriosityEngine::new();
        
        // Always-explore topics should be interesting
        assert!(engine.is_interesting("artificial intelligence").await);
        assert!(engine.is_interesting("machine learning").await);
        
        // Random topic
        assert!(!engine.is_interesting("random topic xyz").await);
    }

    #[test]
    fn test_arxiv_paper() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test Paper".to_string(),
            summary: "A test paper".to_string(),
            authors: vec!["Author 1".to_string(), "Author 2".to_string()],
            published: "2023-01-01".to_string(),
        };
        
        assert_eq!(paper.id, "2301.00001");
        assert_eq!(paper.authors.len(), 2);
    }
}
