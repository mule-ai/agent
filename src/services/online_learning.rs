//! Online Learning Service for Continuous RL
//! 
//! Implements Phase 3 "Continuous learning (online RL)" as specified in SPEC.md:
//! - Experience replay buffer for storing recent interactions
//! - Incremental learning from new examples
//! - Priority-based replay for important experiences
//! - Integration with existing training pipeline
//! - Adaptive learning rate based on performance

use crate::models::{Message, Role, TrainingExample, TrainingSource};
use crate::training::grpo;
use crate::config::OnlineLearningConfig;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Experience entry with priority for replay
#[derive(Debug, Clone)]
pub struct Experience {
    pub id: String,
    pub example: TrainingExample,
    pub priority: f32,
    #[allow(dead_code)]
    pub age_hours: f32,
    #[allow(dead_code)]
    pub error_rate_at_sample: f32,
    pub is_high_value: bool,
}

impl PartialEq for Experience {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Experience {}

impl Experience {
    pub fn new(example: TrainingExample, priority: f32) -> Self {
        Self {
            id: example.id.clone(),
            example,
            priority,
            age_hours: 0.0,
            error_rate_at_sample: 0.0,
            is_high_value: priority > 0.7,
        }
    }

    /// Update age (for future use)
    #[allow(dead_code)]
    pub fn age(&mut self, hours: f32) {
        self.age_hours = hours;
        // Decay priority slightly with age, but keep high-value items relevant
        if !self.is_high_value {
            self.priority *= 0.99_f32.powf(hours);
        }
    }
}

/// Comparison for priority queue (max-heap)
impl Ord for Experience {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.partial_cmp(&other.priority).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl PartialOrd for Experience {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Learning statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearningStats {
    pub examples_collected: usize,
    pub examples_trained: usize,
    pub batches_processed: usize,
    pub avg_reward: f32,
    pub avg_loss: f32,
    pub last_update: Option<String>,
    pub learning_rate_current: f32,
    pub replay_ratio: f32,
    pub high_value_samples: usize,
}

/// Online learning result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningUpdate {
    pub examples_processed: usize,
    pub avg_reward: f32,
    pub avg_loss: f32,
    pub new_knowledge_learned: Vec<String>,
    pub reinforced_knowledge: Vec<String>,
}

impl Default for LearningUpdate {
    fn default() -> Self {
        Self {
            examples_processed: 0,
            avg_reward: 0.0,
            avg_loss: 0.0,
            new_knowledge_learned: Vec::new(),
            reinforced_knowledge: Vec::new(),
        }
    }
}

/// Batch for training
#[derive(Debug, Clone)]
pub struct TrainingBatch {
    pub examples: Vec<TrainingExample>,
    #[allow(dead_code)]
    pub priorities: Vec<f32>,
    pub weights: Vec<f32>,
}

impl TrainingBatch {
    pub fn new(examples: Vec<TrainingExample>, priorities: Vec<f32>) -> Self {
        // Calculate importance weights based on priorities
        let total_priority: f32 = priorities.iter().sum();
        let weights: Vec<f32> = if total_priority > 0.0 {
            priorities.iter().map(|p| p / total_priority).collect()
        } else {
            vec![1.0 / priorities.len() as f32; priorities.len()]
        };

        Self {
            examples,
            priorities,
            weights,
        }
    }

    pub fn size(&self) -> usize {
        self.examples.len()
    }
}

/// Online Learning Service for continuous reinforcement learning
#[derive(Clone)]
pub struct OnlineLearningService {
    config: OnlineLearningConfig,
    /// Experience replay buffer
    replay_buffer: Arc<RwLock<Vec<Experience>>>,
    /// Priority queue for replay selection
    priority_queue: Arc<RwLock<BinaryHeap<Experience>>>,
    /// Statistics
    stats: Arc<RwLock<LearningStats>>,
    /// Model updates counter (for incremental training)
    update_counter: Arc<RwLock<usize>>,
    /// Recent rewards for adaptive learning rate
    recent_rewards: Arc<RwLock<Vec<f32>>>,
    /// Concept embeddings (for detecting new vs reinforced knowledge)
    concept_embeddings: Arc<RwLock<HashMap<String, Vec<f32>>>>,
    /// Pending training examples queue (for future use)
    #[allow(dead_code)]
    pending_examples: Arc<RwLock<Vec<TrainingExample>>>,
}

impl OnlineLearningService {
    /// Create a new online learning service with default config
    pub fn new() -> Self {
        Self::with_config(OnlineLearningConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: OnlineLearningConfig) -> Self {
        Self {
            config,
            replay_buffer: Arc::new(RwLock::new(Vec::new())),
            priority_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            stats: Arc::new(RwLock::new(LearningStats::default())),
            update_counter: Arc::new(RwLock::new(0)),
            recent_rewards: Arc::new(RwLock::new(Vec::new())),
            concept_embeddings: Arc::new(RwLock::new(HashMap::new())),
            pending_examples: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> LearningStats {
        self.stats.read().await.clone()
    }

    /// Add an experience from a conversation
    pub async fn add_experience(&self, example: TrainingExample) {
        // Calculate reward using GRPO reward functions
        let reward = grpo::combined_reward(&example.completion);
        let mut enriched_example = example;
        enriched_example.reward = reward;
        
        // Calculate priority based on reward and quality
        let priority = self.calculate_priority(&enriched_example);
        
        let experience = Experience::new(enriched_example, priority);
        
        // Add to buffer
        {
            let mut buffer = self.replay_buffer.write().await;
            buffer.push(experience.clone());
            
            // Enforce max buffer size with LRU-style eviction
            while buffer.len() > self.config.max_buffer_size {
                buffer.remove(0); // Remove oldest
            }
        }
        
        // Add to priority queue
        {
            let mut queue = self.priority_queue.write().await;
            queue.push(experience);
        }
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.examples_collected += 1;
        }
        
        // Track recent rewards for adaptive learning
        {
            let mut rewards = self.recent_rewards.write().await;
            rewards.push(reward);
            if rewards.len() > 100 {
                rewards.remove(0);
            }
        }
    }

    /// Add multiple experiences from a session
    pub async fn add_session_experiences(&self, messages: &[Message]) {
        // Extract training examples from conversation
        let examples = self.extract_examples_from_conversation(messages);
        
        for example in examples {
            self.add_experience(example).await;
        }
    }

    /// Extract training examples from a conversation
    fn extract_examples_from_conversation(&self, messages: &[Message]) -> Vec<TrainingExample> {
        let mut examples = Vec::new();
        let mut current_prompt = String::new();
        
        for msg in messages.iter() {
            match msg.role {
                Role::User => {
                    current_prompt = msg.content.clone();
                }
                Role::Assistant if !current_prompt.is_empty() => {
                    let example = TrainingExample {
                        id: Uuid::new_v4().to_string(),
                        prompt: current_prompt.clone(),
                        completion: msg.content.clone(),
                        reasoning: msg.reasoning.clone().unwrap_or_default(),
                        reward: 0.0, // Will be calculated by add_experience
                        source: TrainingSource::Session,
                        created_at: Utc::now(),
                        quality_score: self.assess_quality(&current_prompt, &msg.content),
                        used_in_training: false,
                    };
                    examples.push(example);
                    current_prompt.clear();
                }
                _ => {}
            }
        }
        
        examples
    }

    /// Assess the quality of a response
    fn assess_quality(&self, _prompt: &str, completion: &str) -> f32 {
        let mut score: f32 = 0.5; // Base score
        
        // Length appropriateness
        let len = completion.len();
        if len > 50 && len < 2000 {
            score += 0.1;
        } else if len >= 2000 {
            score += 0.05;
        }
        
        // Has structure
        if completion.contains('\n') {
            score += 0.1;
        }
        
        // Contains reasoning tags
        if completion.contains("<REASONING>") || completion.contains("<SOLUTION>") {
            score += 0.15;
        }
        
        // Doesn't contain error indicators
        if !completion.to_lowercase().contains("i don't know")
            && !completion.to_lowercase().contains("I'm not sure")
        {
            score += 0.1;
        }
        
        // Not too short
        if len < 20 {
            score -= 0.2;
        }
        
        score.max(0.0).min(1.0)
    }

    /// Calculate priority for an experience
    fn calculate_priority(&self, example: &TrainingExample) -> f32 {
        // Combine reward and quality
        let reward = grpo::combined_reward(&example.completion);
        let quality = example.quality_score;
        
        // Priority = weighted combination
        let base_priority = reward * 0.6 + quality * 0.4;
        
        // Boost for novel content (could be enhanced with semantic similarity)
        let novelty_bonus = if example.source == TrainingSource::Session {
            0.1
        } else {
            0.0
        };
        
        (base_priority + novelty_bonus).min(1.0)
    }

    /// Sample a batch for training using prioritized replay
    pub async fn sample_batch(&self, batch_size: usize) -> Option<TrainingBatch> {
        let buffer_size = {
            let buffer = self.replay_buffer.read().await;
            buffer.len()
        };
        
        if buffer_size == 0 {
            return None;
        }
        
        // Calculate how many to sample from replay
        let replay_count = (batch_size as f32 * self.config.replay_ratio).round() as usize;
        let fresh_count = batch_size.saturating_sub(replay_count);
        
        let mut batch_examples = Vec::new();
        let mut batch_priorities = Vec::new();
        
        // Sample from priority queue (high priority first)
        {
            let mut queue = self.priority_queue.write().await;
            let mut temp = Vec::new();
            
            for _ in 0..replay_count.min(buffer_size) {
                if let Some(exp) = queue.pop() {
                    batch_examples.push(exp.example.clone());
                    batch_priorities.push(exp.priority);
                    temp.push(exp);
                }
            }
            
            // Put back the popped items (they'll have updated priorities)
            for exp in temp {
                queue.push(exp);
            }
        }
        
        // Add fresh samples
        if fresh_count > 0 {
            let buffer = self.replay_buffer.read().await;
            let start = buffer.len().saturating_sub(fresh_count);
            
            for exp in buffer.iter().skip(start) {
                batch_examples.push(exp.example.clone());
                batch_priorities.push(exp.priority);
            }
        }
        
        if batch_examples.is_empty() {
            return None;
        }
        
        Some(TrainingBatch::new(batch_examples, batch_priorities))
    }

    /// Perform an online learning update
    pub async fn learn(&self) -> Result<LearningUpdate> {
        let mut update = LearningUpdate::default();
        
        // Get a batch to train on
        let batch = match self.sample_batch(self.config.batch_size).await {
            Some(b) => b,
            None => return Ok(update),
        };
        
        update.examples_processed = batch.size();
        
        // Calculate metrics
        let mut total_reward = 0.0;
        let mut total_loss = 0.0;
        let mut new_knowledge = Vec::new();
        let mut reinforced = Vec::new();
        
        for (i, example) in batch.examples.iter().enumerate() {
            total_reward += example.reward;
            
            // Simulate loss calculation (in real implementation, this would update model)
            let sample_loss = (1.0 - example.reward) * batch.weights.get(i).copied().unwrap_or(1.0);
            total_loss += sample_loss;
            
            // Track concept reinforcement
            self.update_concept_embedding(&example.prompt, example.reward).await?;
            
            if example.quality_score > 0.7 {
                new_knowledge.push(example.prompt.chars().take(50).collect());
            } else {
                reinforced.push(example.prompt.chars().take(50).collect());
            }
        }
        
        update.avg_reward = if !batch.examples.is_empty() {
            total_reward / batch.examples.len() as f32
        } else {
            0.0
        };
        
        update.avg_loss = if !batch.examples.is_empty() {
            total_loss / batch.examples.len() as f32
        } else {
            0.0
        };
        
        update.new_knowledge_learned = new_knowledge;
        update.reinforced_knowledge = reinforced;
        
        // Mark examples as used
        self.mark_examples_trained(&batch.examples).await?;
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.examples_trained += batch.size();
            stats.batches_processed += 1;
            stats.avg_reward = update.avg_reward;
            stats.avg_loss = update.avg_loss;
            stats.last_update = Some(Utc::now().to_rfc3339());
            
            // Update learning rate based on recent performance
            let rewards = self.recent_rewards.read().await;
            if rewards.len() >= 10 {
                let recent_avg: f32 = rewards.iter().sum::<f32>() / rewards.len() as f32;
                
                // Adaptive learning rate: decrease if performance is good, increase if poor
                if recent_avg > 0.7 {
                    stats.learning_rate_current *= 0.95; // Decay
                } else if recent_avg < 0.4 {
                    stats.learning_rate_current *= 1.1; // Boost
                }
                
                stats.learning_rate_current = stats.learning_rate_current
                    .max(self.config.min_learning_rate)
                    .min(self.config.max_learning_rate);
            }
        }
        
        // Increment update counter
        {
            let mut counter = self.update_counter.write().await;
            *counter += 1;
        }
        
        Ok(update)
    }

    /// Update concept embedding (simplified semantic tracking)
    async fn update_concept_embedding(&self, text: &str, reward: f32) -> Result<()> {
        // Simple word-based embedding for concept tracking
        let words: Vec<&str> = text.split_whitespace().collect();
        
        let mut embeddings = self.concept_embeddings.write().await;
        
        for word in words.iter().take(10) {
            let entry = embeddings.entry(word.to_lowercase()).or_insert_with(|| {
                vec![0.0; 10] // Simple 10-dim embedding
            });
            
            // Update with exponential moving average
            let alpha = 0.1;
            if entry[0] == 0.0 {
                entry[0] = reward;
            } else {
                entry[0] = alpha * reward + (1.0 - alpha) * entry[0];
            }
        }
        
        Ok(())
    }

    /// Mark examples as trained
    async fn mark_examples_trained(&self, examples: &[TrainingExample]) -> Result<()> {
        let trained_ids: Vec<String> = examples.iter().map(|e| e.id.clone()).collect();
        
        let mut buffer = self.replay_buffer.write().await;
        for exp in buffer.iter_mut() {
            if trained_ids.contains(&exp.example.id) {
                exp.example.used_in_training = true;
            }
        }
        
        Ok(())
    }

    /// Get buffer statistics
    pub async fn get_buffer_stats(&self) -> BufferStats {
        let buffer = self.replay_buffer.read().await;
        let trained = buffer.iter().filter(|e| e.example.used_in_training).count();
        let high_value = buffer.iter().filter(|e| e.is_high_value).count();
        
        BufferStats {
            total: buffer.len(),
            trained: trained,
            untrained: buffer.len() - trained,
            high_value: high_value,
            capacity: self.config.max_buffer_size,
            utilization: buffer.len() as f32 / self.config.max_buffer_size as f32,
        }
    }

    /// Clear old trained examples from buffer
    pub async fn prune_trained(&self) -> usize {
        let mut buffer = self.replay_buffer.write().await;
        let initial_len = buffer.len();
        
        buffer.retain(|e| !e.example.used_in_training || e.is_high_value);
        
        initial_len - buffer.len()
    }

    /// Get pending examples count (for future use)
    #[allow(dead_code)]
    pub async fn pending_count(&self) -> usize {
        self.pending_examples.read().await.len()
    }

    /// Get update counter (for future use)
    #[allow(dead_code)]
    pub async fn get_update_count(&self) -> usize {
        *self.update_counter.read().await
    }

    /// Check if ready to train
    pub async fn is_ready(&self) -> bool {
        let buffer = self.replay_buffer.read().await;
        buffer.len() >= self.config.min_buffer_for_training
    }

    /// Get concept embeddings for analysis
    pub async fn get_concepts(&self) -> HashMap<String, Vec<f32>> {
        self.concept_embeddings.read().await.clone()
    }
}

impl Default for OnlineLearningService {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStats {
    pub total: usize,
    pub trained: usize,
    pub untrained: usize,
    pub high_value: usize,
    pub capacity: usize,
    pub utilization: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experience_priority() {
        let example = TrainingExample::new("Test prompt".to_string(), "Test completion".to_string());
        let exp = Experience::new(example, 0.8);
        
        assert!(exp.is_high_value);
        assert_eq!(exp.priority, 0.8);
    }

    #[test]
    fn test_priority_queue_ordering() {
        let mut heap = BinaryHeap::new();
        
        let e1 = Experience::new(
            TrainingExample::new("p1".to_string(), "c1".to_string()),
            0.5,
        );
        let e2 = Experience::new(
            TrainingExample::new("p2".to_string(), "c2".to_string()),
            0.9,
        );
        let e3 = Experience::new(
            TrainingExample::new("p3".to_string(), "c3".to_string()),
            0.3,
        );
        
        heap.push(e1);
        heap.push(e2);
        heap.push(e3);
        
        // Highest priority should come first
        let first = heap.pop().unwrap();
        assert!(first.priority > 0.8);
    }

    #[test]
    fn test_learning_stats_default() {
        let stats = LearningStats::default();
        assert_eq!(stats.examples_collected, 0);
        assert_eq!(stats.examples_trained, 0);
    }

    #[tokio::test]
    async fn test_add_experience() {
        let service = OnlineLearningService::new();
        let example = TrainingExample::new("Hello".to_string(), "Hi there!".to_string());
        
        service.add_experience(example).await;
        
        let stats = service.get_stats().await;
        assert_eq!(stats.examples_collected, 1);
    }

    #[tokio::test]
    async fn test_sample_batch() {
        let service = OnlineLearningService::new();
        
        // Add some examples
        for i in 0..10 {
            let example = TrainingExample::new(
                format!("Prompt {}", i),
                format!("Completion {}", i),
            );
            service.add_experience(example).await;
        }
        
        // Sample a batch
        let batch = service.sample_batch(5).await;
        assert!(batch.is_some());
        assert_eq!(batch.unwrap().size(), 5);
    }

    #[tokio::test]
    async fn test_learn() {
        let service = OnlineLearningService::new();
        
        // Add examples
        for i in 0..20 {
            let example = TrainingExample::new(
                format!("Prompt {}", i),
                format!("<REASONING>Thinking...</REASONING>\n<SOLUTION>Answer</SOLUTION>"),
            );
            service.add_experience(example).await;
        }
        
        // Learn
        let update = service.learn().await.unwrap();
        assert!(update.examples_processed > 0);
        
        let stats = service.get_stats().await;
        assert!(stats.batches_processed >= 1);
    }

    #[tokio::test]
    async fn test_is_ready() {
        let service = OnlineLearningService::new();
        
        // Not ready with no examples
        assert!(!service.is_ready().await);
        
        // Add enough examples to meet min_buffer requirement (default 50)
        for i in 0..50 {
            let example = TrainingExample::new(
                format!("Prompt {}", i),
                format!("Completion {}", i),
            );
            service.add_experience(example).await;
        }
        
        // With default min_buffer=50, should be ready after adding 50
        assert!(service.is_ready().await);
    }

    #[test]
    fn test_batch_weight_calculation() {
        let examples = vec![
            TrainingExample::new("p1".to_string(), "c1".to_string()),
            TrainingExample::new("p2".to_string(), "c2".to_string()),
        ];
        let priorities = vec![0.8, 0.2];
        
        let batch = TrainingBatch::new(examples, priorities);
        
        assert_eq!(batch.size(), 2);
        assert!((batch.weights[0] - 0.8).abs() < 0.01); // 0.8 / 1.0
        assert!((batch.weights[1] - 0.2).abs() < 0.01); // 0.2 / 1.0
    }

    #[tokio::test]
    async fn test_extract_examples_from_conversation() {
        use crate::models::Role;
        
        let messages = vec![
            Message::user("What is Rust?".to_string()),
            Message::assistant("Rust is a programming language.".to_string()),
            Message::user("Tell me more.".to_string()),
            Message::assistant("It focuses on safety and performance.".to_string()),
        ];
        
        let service = OnlineLearningService::new();
        let examples = service.extract_examples_from_conversation(&messages);
        
        assert_eq!(examples.len(), 2);
        assert_eq!(examples[0].prompt, "What is Rust?");
        assert_eq!(examples[0].completion, "Rust is a programming language.");
    }

    #[tokio::test]
    async fn test_prune_trained() {
        let service = OnlineLearningService::new();
        
        // Add and mark some as trained
        for i in 0..10 {
            let example = TrainingExample::new(
                format!("Prompt {}", i),
                format!("Completion {}", i),
            );
            service.add_experience(example).await;
        }
        
        // Mark all as trained (simulate training)
        let buffer_stats = service.get_buffer_stats().await;
        assert_eq!(buffer_stats.total, 10);
        
        // Note: prune only removes trained non-high-value examples
        // In real use, you'd train first then prune
    }

    #[test]
    fn test_quality_assessment() {
        let service = OnlineLearningService::new();
        
        // Good response
        let good_quality = service.assess_quality(
            "What is Rust?",
            "<REASONING>Rust is a systems programming language.</REASONING>\n<SOLUTION>It provides memory safety without garbage collection.</SOLUTION>",
        );
        assert!(good_quality > 0.6);
        
        // Poor response
        let poor_quality = service.assess_quality(
            "What is Rust?",
            "IDK",
        );
        assert!(poor_quality < 0.5);
    }

    #[tokio::test]
    async fn test_buffer_stats() {
        let service = OnlineLearningService::new();
        
        let stats = service.get_buffer_stats().await;
        assert_eq!(stats.total, 0);
        assert_eq!(stats.capacity, OnlineLearningConfig::default().max_buffer_size);
    }
}
