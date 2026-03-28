//! Memory eviction policies
//! 
//! Implements memory eviction as specified in SPEC.md:
//! - TTL-based expiration
//! - Quality scoring
//! - Move concepts to training, delete transient facts

use crate::models::{Memory, MemoryType};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

/// Eviction policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionPolicy {
    /// Maximum age for memories in hours
    pub max_age_hours: u32,
    /// Minimum quality score to keep
    pub min_quality_score: f32,
    /// Whether to evict concepts
    pub evict_concepts: bool,
    /// Whether to keep facts (but maybe move to training)
    pub keep_facts: bool,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self {
            max_age_hours: 24,
            min_quality_score: 0.3,
            evict_concepts: true,
            keep_facts: true,
        }
    }
}

impl EvictionPolicy {
    /// Create a new eviction policy with defaults
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum age in hours
    #[allow(dead_code)]
    pub fn with_max_age(mut self, hours: u32) -> Self {
        self.max_age_hours = hours;
        self
    }

    /// Set minimum quality score
    #[allow(dead_code)]
    pub fn with_min_quality(mut self, score: f32) -> Self {
        self.min_quality_score = score;
        self
    }

    /// Set whether to evict concepts
    #[allow(dead_code)]
    pub fn with_evict_concepts(mut self, evict: bool) -> Self {
        self.evict_concepts = evict;
        self
    }

    /// Set whether to keep facts
    #[allow(dead_code)]
    pub fn with_keep_facts(mut self, keep: bool) -> Self {
        self.keep_facts = keep;
        self
    }
}

/// Memory eviction decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvictionDecision {
    /// Keep the memory
    Keep,
    /// Move to training namespace
    MoveToTraining,
    /// Delete the memory
    Delete,
}

/// Memory eviction service
#[allow(dead_code)]
pub struct MemoryEviction {
    policy: EvictionPolicy,
}

#[allow(dead_code)]
impl MemoryEviction {
    /// Create a new memory eviction service
    pub fn new(policy: EvictionPolicy) -> Self {
        Self { policy }
    }

    /// Create with default policy
    pub fn default() -> Self {
        Self {
            policy: EvictionPolicy::default(),
        }
    }

    /// Decide what to do with a memory
    pub fn evaluate(&self, memory: &Memory, access_count: usize) -> EvictionDecision {
        // Check if memory is too old
        if self.is_expired(memory) {
            return self.decide_for_expired(memory);
        }

        // Check quality score
        let quality = self.calculate_quality_score(memory, access_count);
        if quality < self.policy.min_quality_score {
            return EvictionDecision::Delete;
        }

        // Keep high quality memories
        EvictionDecision::Keep
    }

    /// Check if a memory has expired based on TTL
    pub fn is_expired(&self, memory: &Memory) -> bool {
        let max_age = Duration::hours(self.policy.max_age_hours as i64);
        let age = Utc::now() - memory.created_at;
        age > max_age
    }

    /// Decide what to do with an expired memory
    fn decide_for_expired(&self, memory: &Memory) -> EvictionDecision {
        match memory.memory_type {
            MemoryType::Concept => {
                // Concepts should move to training
                if self.policy.evict_concepts {
                    EvictionDecision::MoveToTraining
                } else {
                    EvictionDecision::Keep
                }
            }
            MemoryType::Fact => {
                // Facts can be deleted unless persistent
                if self.policy.keep_facts && !memory.is_persistent {
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

    /// Calculate quality score for a memory
    /// 
    /// Factors:
    /// - Novelty: Is this new information?
    /// - Utility: Will this be useful?
    /// - Generalizability: Can this be abstracted?
    pub fn calculate_quality_score(&self, memory: &Memory, access_count: usize) -> f32 {
        let mut score: f32 = 0.5; // Base score

        // Boost for concepts (more likely to be useful)
        if memory.memory_type == MemoryType::Concept {
            score += 0.2;
        }

        // Boost for frequent access (indicates utility)
        if access_count > 5 {
            score += 0.2;
        } else if access_count > 2 {
            score += 0.1;
        }

        // Boost for persistent memories
        if memory.is_persistent {
            score += 0.15;
        }

        // Boost for having tags (structured knowledge)
        if !memory.tags.is_empty() {
            score += 0.1;
        }

        // Boost for being marked to move to training
        if memory.evict_to_training {
            score += 0.1;
        }

        // Clamp to [0, 1]
        score.max(0.0).min(1.0)
    }

    /// Process a batch of memories for eviction
    pub fn process_batch(&self, memories: &mut [Memory], access_counts: &[usize]) -> Vec<EvictionResult> {
        assert_eq!(memories.len(), access_counts.len());

        memories
            .iter_mut()
            .zip(access_counts.iter())
            .map(|(memory, count)| {
                let decision = self.evaluate(memory, *count);
                let result = EvictionResult {
                    memory_id: memory.id.clone(),
                    decision: decision.clone(),
                    reason: self.get_decision_reason(&decision, memory),
                };
                
                // Apply the decision
                match decision {
                    EvictionDecision::MoveToTraining => {
                        memory.namespace = "training".to_string();
                        memory.is_persistent = true;
                    }
                    EvictionDecision::Delete => {
                        // Memory will be deleted by caller
                    }
                    EvictionDecision::Keep => {}
                }

                result
            })
            .collect()
    }

    /// Get human-readable reason for eviction decision
    fn get_decision_reason(&self, decision: &EvictionDecision, memory: &Memory) -> String {
        match decision {
            EvictionDecision::Keep => "Memory is useful and should be retained".to_string(),
            EvictionDecision::MoveToTraining => {
                format!("Concept '{}' should be learned for training", memory.memory_type())
            }
            EvictionDecision::Delete => {
                if self.is_expired(memory) {
                    format!("Memory expired after {} hours", self.policy.max_age_hours)
                } else {
                    format!("Memory quality too low")
                }
            }
        }
    }
}

impl Memory {
    #[allow(dead_code)]
    fn memory_type(&self) -> &str {
        match self.memory_type {
            MemoryType::Fact => "fact",
            MemoryType::Concept => "concept",
            MemoryType::Conversation => "conversation",
            MemoryType::ToolResult => "tool_result",
        }
    }
}

/// Result of eviction evaluation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EvictionResult {
    pub memory_id: String,
    pub decision: EvictionDecision,
    pub reason: String,
}

/// Statistics about eviction
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct EvictionStats {
    pub evaluated: usize,
    pub kept: usize,
    pub moved_to_training: usize,
    pub deleted: usize,
}

impl EvictionStats {
    #[allow(dead_code)]
    pub fn from_results(results: &[EvictionResult]) -> Self {
        let mut stats = Self::default();
        stats.evaluated = results.len();
        
        for result in results {
            match result.decision {
                EvictionDecision::Keep => stats.kept += 1,
                EvictionDecision::MoveToTraining => stats.moved_to_training += 1,
                EvictionDecision::Delete => stats.deleted += 1,
            }
        }
        
        stats
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
    fn test_default_policy() {
        let policy = EvictionPolicy::default();
        assert_eq!(policy.max_age_hours, 24);
        assert_eq!(policy.min_quality_score, 0.3);
        assert!(policy.evict_concepts);
        assert!(policy.keep_facts);
    }

    #[test]
    fn test_policy_builder() {
        let policy = EvictionPolicy::new()
            .with_max_age(48)
            .with_min_quality(0.5)
            .with_evict_concepts(false)
            .with_keep_facts(false);

        assert_eq!(policy.max_age_hours, 48);
        assert_eq!(policy.min_quality_score, 0.5);
        assert!(!policy.evict_concepts);
        assert!(!policy.keep_facts);
    }

    #[test]
    fn test_concept_moved_to_training() {
        let eviction = MemoryEviction::default();
        let memory = create_test_memory(MemoryType::Concept, 25); // Expired

        let decision = eviction.evaluate(&memory, 0);
        assert_eq!(decision, EvictionDecision::MoveToTraining);
    }

    #[test]
    fn test_fact_deleted_when_expired() {
        let eviction = MemoryEviction::default();
        let memory = create_test_memory(MemoryType::Fact, 25); // Expired

        let decision = eviction.evaluate(&memory, 0);
        assert_eq!(decision, EvictionDecision::Delete);
    }

    #[test]
    fn test_persistent_fact_kept() {
        let mut eviction = MemoryEviction::default();
        eviction.policy.keep_facts = true;
        
        let mut memory = create_test_memory(MemoryType::Fact, 25);
        memory.is_persistent = true;

        let decision = eviction.evaluate(&memory, 0);
        assert_eq!(decision, EvictionDecision::Keep);
    }

    #[test]
    fn test_conversation_deleted() {
        let eviction = MemoryEviction::default();
        let memory = create_test_memory(MemoryType::Conversation, 25); // Expired

        let decision = eviction.evaluate(&memory, 0);
        // Expired conversations are deleted
        assert_eq!(decision, EvictionDecision::Delete);
    }

    #[test]
    fn test_conversation_kept_when_fresh() {
        let eviction = MemoryEviction::default();
        let memory = create_test_memory(MemoryType::Conversation, 1); // Not expired

        let decision = eviction.evaluate(&memory, 0);
        // Fresh conversations are kept
        assert_eq!(decision, EvictionDecision::Keep);
    }

    #[test]
    fn test_quality_score_calculation() {
        let eviction = MemoryEviction::default();

        // Low quality memory
        let low_quality = create_test_memory(MemoryType::Fact, 1);
        let score = eviction.calculate_quality_score(&low_quality, 0);
        assert!(score < 0.7);

        // High quality memory (concept with tags and access)
        let mut high_quality = create_test_memory(MemoryType::Concept, 1);
        high_quality.tags = vec!["important".to_string()];
        high_quality.is_persistent = true;
        let score = eviction.calculate_quality_score(&high_quality, 10);
        assert!(score > 0.7);
    }

    #[test]
    fn test_expired_check() {
        let eviction = MemoryEviction::default();

        // Fresh memory
        let fresh = create_test_memory(MemoryType::Fact, 0);
        assert!(!eviction.is_expired(&fresh));

        // Old memory
        let old = create_test_memory(MemoryType::Fact, 48);
        assert!(eviction.is_expired(&old));
    }

    #[test]
    fn test_batch_processing() {
        let eviction = MemoryEviction::default();
        
        let mut memories = vec![
            create_test_memory(MemoryType::Concept, 25), // Move to training (expired concept)
            create_test_memory(MemoryType::Fact, 25),   // Delete (expired fact)
            create_test_memory(MemoryType::Conversation, 25), // Delete (expired conversation)
            create_test_memory(MemoryType::Concept, 1),  // Keep (not expired, good quality)
        ];
        let access_counts = vec![0, 0, 0, 10];

        let results = eviction.process_batch(&mut memories, &access_counts);

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].decision, EvictionDecision::MoveToTraining);
        assert_eq!(results[1].decision, EvictionDecision::Delete);
        assert_eq!(results[2].decision, EvictionDecision::Delete);
        assert_eq!(results[3].decision, EvictionDecision::Keep);
    }

    #[test]
    fn test_eviction_stats() {
        let results = vec![
            EvictionResult {
                memory_id: "1".to_string(),
                decision: EvictionDecision::Keep,
                reason: "".to_string(),
            },
            EvictionResult {
                memory_id: "2".to_string(),
                decision: EvictionDecision::MoveToTraining,
                reason: "".to_string(),
            },
            EvictionResult {
                memory_id: "3".to_string(),
                decision: EvictionDecision::Delete,
                reason: "".to_string(),
            },
        ];

        let stats = EvictionStats::from_results(&results);
        assert_eq!(stats.evaluated, 3);
        assert_eq!(stats.kept, 1);
        assert_eq!(stats.moved_to_training, 1);
        assert_eq!(stats.deleted, 1);
    }
}
