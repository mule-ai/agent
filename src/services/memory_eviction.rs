//! Memory Eviction Service
//! 
//! Implements memory eviction as specified in SPEC.md:
//! - Periodic processing of expired memories
//! - Move concepts to training namespace
//! - Delete transient facts
//! - Track eviction statistics

use crate::memory::eviction::{EvictionPolicy, EvictionDecision};
use crate::models::{Memory, MemoryType};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for memory eviction service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvictionConfig {
    /// Enable memory eviction service
    pub enabled: bool,
    /// Namespaces to process
    pub namespaces: Vec<String>,
    /// Eviction policy
    pub eviction_policy: EvictionPolicyConfig,
    /// Processing interval in seconds
    pub processing_interval_seconds: u64,
}

impl Default for MemoryEvictionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            namespaces: vec!["retrieval".to_string()],
            eviction_policy: EvictionPolicyConfig::default(),
            processing_interval_seconds: 3600, // 1 hour
        }
    }
}

/// Eviction policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionPolicyConfig {
    /// Maximum age for memories in hours
    pub max_age_hours: u32,
    /// Minimum quality score to keep
    pub min_quality_score: f32,
    /// Whether to evict concepts
    pub evict_concepts: bool,
    /// Whether to keep facts
    pub keep_facts: bool,
}

impl Default for EvictionPolicyConfig {
    fn default() -> Self {
        Self {
            max_age_hours: 24,
            min_quality_score: 0.3,
            evict_concepts: true,
            keep_facts: true,
        }
    }
}

impl From<&EvictionPolicyConfig> for EvictionPolicy {
    fn from(config: &EvictionPolicyConfig) -> Self {
        EvictionPolicy {
            max_age_hours: config.max_age_hours,
            min_quality_score: config.min_quality_score,
            evict_concepts: config.evict_concepts,
            keep_facts: config.keep_facts,
        }
    }
}

/// Eviction result for a single memory
#[derive(Debug, Clone, Serialize)]
pub struct MemoryEvictionResult {
    pub memory_id: String,
    pub action: String,
    pub reason: String,
}

/// Service statistics
#[derive(Debug, Clone, Default, Serialize)]
pub struct MemoryEvictionStats {
    pub total_processed: usize,
    pub kept: usize,
    pub moved_to_training: usize,
    pub deleted: usize,
    pub errors: usize,
    pub last_run: Option<String>,
}

impl MemoryEvictionStats {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn add_kept(&mut self) {
        self.kept += 1;
    }

    #[allow(dead_code)]
    pub fn add_moved(&mut self) {
        self.moved_to_training += 1;
    }

    #[allow(dead_code)]
    pub fn add_deleted(&mut self) {
        self.deleted += 1;
    }

    #[allow(dead_code)]
    pub fn add_error(&mut self) {
        self.errors += 1;
    }

    #[allow(dead_code)]
    pub fn finish_run(&mut self) {
        self.last_run = Some(Utc::now().to_rfc3339());
    }
}

/// Memory eviction service
#[derive(Clone)]
pub struct MemoryEvictionService {
    config: MemoryEvictionConfig,
    stats: Arc<RwLock<MemoryEvictionStats>>,
}

impl MemoryEvictionService {
    pub fn new() -> Self {
        Self {
            config: MemoryEvictionConfig::default(),
            stats: Arc::new(RwLock::new(MemoryEvictionStats::new())),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: MemoryEvictionConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(MemoryEvictionStats::new())),
        }
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> MemoryEvictionStats {
        self.stats.read().await.clone()
    }

    /// Check if a memory is expired
    pub fn is_memory_expired(&self, memory: &Memory) -> bool {
        let max_age = Duration::hours(self.config.eviction_policy.max_age_hours as i64);
        let age = Utc::now() - memory.created_at;
        age > max_age
    }

    /// Determine eviction action for a memory
    pub fn evaluate_memory(&self, memory: &Memory, _access_count: usize) -> EvictionDecision {
        // Check if memory is too old
        if self.is_memory_expired(memory) {
            return self.decide_for_expired(memory);
        }

        // For now, keep memories that aren't expired
        EvictionDecision::Keep
    }

    /// Decide what to do with an expired memory
    fn decide_for_expired(&self, memory: &Memory) -> EvictionDecision {
        match memory.memory_type {
            MemoryType::Concept => {
                // Concepts should move to training
                if self.config.eviction_policy.evict_concepts {
                    EvictionDecision::MoveToTraining
                } else {
                    EvictionDecision::Keep
                }
            }
            MemoryType::Fact => {
                // Facts can be deleted unless persistent
                if self.config.eviction_policy.keep_facts && !memory.is_persistent {
                    EvictionDecision::Delete
                } else {
                    EvictionDecision::Keep
                }
            }
            MemoryType::Conversation => {
                // Conversations are typically transient
                EvictionDecision::Delete
            }
            MemoryType::ToolResult => {
                // Tool results are transient
                EvictionDecision::Delete
            }
        }
    }

    /// Process a single memory and return the result
    pub fn process_memory(&self, memory: &Memory) -> MemoryEvictionResult {
        let decision = self.evaluate_memory(memory, 0);
        
        let action = match decision {
            EvictionDecision::Keep => "kept".to_string(),
            EvictionDecision::MoveToTraining => "moved_to_training".to_string(),
            EvictionDecision::Delete => "deleted".to_string(),
        };

        let reason = match decision {
            EvictionDecision::Keep => {
                if self.is_memory_expired(memory) {
                    format!("Memory is {} but kept due to type", memory.memory_type_str())
                } else {
                    "Memory is still valid".to_string()
                }
            }
            EvictionDecision::MoveToTraining => {
                format!("Concept '{}' should be learned for training", memory.memory_type_str())
            }
            EvictionDecision::Delete => {
                if self.is_memory_expired(memory) {
                    format!("Memory expired after {} hours", self.config.eviction_policy.max_age_hours)
                } else {
                    "Memory quality too low".to_string()
                }
            }
        };

        MemoryEvictionResult {
            memory_id: memory.id.clone(),
            action,
            reason,
        }
    }

    #[allow(dead_code)]
    pub async fn process_batch(&self, memories: &mut [Memory]) -> Vec<MemoryEvictionResult> {
        let mut results = Vec::new();
        let mut stats = self.stats.write().await;

        for memory in memories.iter_mut() {
            stats.total_processed += 1;
            
            let result = self.process_memory(memory);
            
            // Update memory based on decision
            match result.action.as_str() {
                "moved_to_training" => {
                    memory.namespace = "training".to_string();
                    memory.is_persistent = true;
                    stats.add_moved();
                }
                "deleted" => {
                    stats.add_deleted();
                }
                _ => {
                    stats.add_kept();
                }
            }
            
            results.push(result);
        }

        stats.finish_run();
        results
    }

    /// Get expired memories from a list
    pub fn get_expired_memories<'a>(&self, memories: &'a [Memory]) -> Vec<&'a Memory> {
        memories
            .iter()
            .filter(|m| self.is_memory_expired(m))
            .collect()
    }

    /// Categorize memories by eviction action
    pub fn categorize_memories(&self, memories: &[Memory]) -> MemoryCategories {
        let mut categories = MemoryCategories::default();
        
        for memory in memories {
            let decision = self.evaluate_memory(memory, 0);
            
            match decision {
                EvictionDecision::Keep => categories.keep.push(memory.id.clone()),
                EvictionDecision::MoveToTraining => categories.move_to_training.push(memory.id.clone()),
                EvictionDecision::Delete => categories.delete.push(memory.id.clone()),
            }
        }
        
        categories
    }
}

impl Default for MemoryEvictionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Categories of memories based on eviction action
#[derive(Debug, Clone, Default, Serialize)]
#[allow(dead_code)]
pub struct MemoryCategories {
    pub keep: Vec<String>,
    pub move_to_training: Vec<String>,
    pub delete: Vec<String>,
}

impl Memory {
    /// Get human-readable memory type
    pub fn memory_type_str(&self) -> &str {
        match self.memory_type {
            MemoryType::Fact => "fact",
            MemoryType::Concept => "concept",
            MemoryType::Conversation => "conversation",
            MemoryType::ToolResult => "tool_result",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_memory(memory_type: MemoryType, hours_old: i64) -> Memory {
        let created_at = Utc::now() - Duration::hours(hours_old);
        Memory {
            id: "test".to_string(),
            content: "Test content".to_string(),
            embedding: Vec::new(),
            namespace: "retrieval".to_string(),
            tags: vec!["test".to_string()],
            metadata: std::collections::HashMap::new(),
            created_at,
            updated_at: created_at,
            memory_type,
            evict_to_training: false,
            is_persistent: false,
        }
    }

    #[test]
    fn test_service_creation() {
        let service = MemoryEvictionService::new();
        assert!(service.config.enabled);
        assert_eq!(service.config.namespaces, vec!["retrieval".to_string()]);
    }

    #[test]
    fn test_memory_expiration() {
        let service = MemoryEvictionService::new();
        
        // Fresh memory
        let fresh = create_test_memory(MemoryType::Fact, 0);
        assert!(!service.is_memory_expired(&fresh));
        
        // Old memory (25 hours)
        let old = create_test_memory(MemoryType::Fact, 25);
        assert!(service.is_memory_expired(&old));
    }

    #[test]
    fn test_concept_moved_to_training() {
        let service = MemoryEvictionService::new();
        let concept = create_test_memory(MemoryType::Concept, 25);
        
        let decision = service.evaluate_memory(&concept, 0);
        assert_eq!(decision, EvictionDecision::MoveToTraining);
    }

    #[test]
    fn test_fact_deleted_when_expired() {
        let service = MemoryEvictionService::new();
        let fact = create_test_memory(MemoryType::Fact, 25);
        
        let decision = service.evaluate_memory(&fact, 0);
        assert_eq!(decision, EvictionDecision::Delete);
    }

    #[test]
    fn test_persistent_fact_kept() {
        let service = MemoryEvictionService::new();
        let mut fact = create_test_memory(MemoryType::Fact, 25);
        fact.is_persistent = true;
        
        let decision = service.evaluate_memory(&fact, 0);
        assert_eq!(decision, EvictionDecision::Keep);
    }

    #[test]
    fn test_process_memory() {
        let service = MemoryEvictionService::new();
        
        let concept = create_test_memory(MemoryType::Concept, 25);
        let result = service.process_memory(&concept);
        
        assert_eq!(result.memory_id, "test");
        assert_eq!(result.action, "moved_to_training");
    }

    #[test]
    fn test_categorize_memories() {
        let service = MemoryEvictionService::new();
        
        let memories = vec![
            create_test_memory(MemoryType::Concept, 25),  // Move to training
            create_test_memory(MemoryType::Fact, 25),    // Delete
            create_test_memory(MemoryType::Fact, 1),     // Keep (not expired)
            create_test_memory(MemoryType::Conversation, 25), // Delete
        ];
        
        let categories = service.categorize_memories(&memories);
        
        assert_eq!(categories.keep.len(), 1);
        assert_eq!(categories.move_to_training.len(), 1);
        assert_eq!(categories.delete.len(), 2);
    }

    #[test]
    fn test_stats_tracking() {
        let service = MemoryEvictionService::new();
        
        let mut memories = vec![
            create_test_memory(MemoryType::Concept, 25),
            create_test_memory(MemoryType::Fact, 25),
            create_test_memory(MemoryType::Fact, 1),
        ];
        
        let _results = tokio_test::block_on(service.process_batch(&mut memories));
        
        let stats = tokio_test::block_on(service.get_stats());
        
        assert_eq!(stats.total_processed, 3);
        assert_eq!(stats.moved_to_training, 1);
        assert_eq!(stats.deleted, 1);
        assert_eq!(stats.kept, 1);
        assert!(stats.last_run.is_some());
    }
}
