//! Memory system for AGI Agent
//! 
//! Implements the memory system as specified in SPEC.md:
//! - Short-term retrieval memory (SQLite + Tantivy)
//! - Long-term training memory
//! - Automatic embedding
//! - TTL-based eviction

pub mod embedding;
pub mod store;
pub mod retrieval;
pub mod eviction;

pub use embedding::EmbeddingClient;
pub use store::SqliteMemoryStore;
#[allow(unused)]
pub use eviction::EvictionPolicy;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Memory, MemoryType};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_store_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let index_path = temp_dir.path().join("index");
        
        let store = SqliteMemoryStore::new(&db_path, &index_path).unwrap();
        
        // Create a memory
        let mut memory = Memory::new(
            "Test memory content".to_string(),
            "retrieval".to_string(),
        );
        memory.tags = vec!["test".to_string()];
        memory.memory_type = MemoryType::Fact;
        memory.embedding = vec![0.1, 0.2, 0.3];
        
        // Store
        store.store(&memory).unwrap();
        
        // List
        let memories = store.list("retrieval", 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "Test memory content");
        
        // Delete
        store.delete(&memory.id).unwrap();
        
        let memories = store.list("retrieval", 10).unwrap();
        assert!(memories.is_empty());
    }

    #[tokio::test]
    async fn test_memory_namespaces() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let index_path = temp_dir.path().join("index");
        
        let store = SqliteMemoryStore::new(&db_path, &index_path).unwrap();
        
        let memory1 = Memory::with_params(
            "Retrieval content".to_string(),
            "retrieval".to_string(),
            vec![],
            Some(MemoryType::Fact),
            false,
        );
        
        let memory2 = Memory::with_params(
            "Training content".to_string(),
            "training".to_string(),
            vec![],
            Some(MemoryType::Concept),
            true,
        );
        
        store.store(&memory1).unwrap();
        store.store(&memory2).unwrap();
        
        let retrieval_memories = store.list("retrieval", 10).unwrap();
        assert_eq!(retrieval_memories.len(), 1);
        
        let training_memories = store.list("training", 10).unwrap();
        assert_eq!(training_memories.len(), 1);
    }

    #[test]
    fn test_eviction_policy_default() {
        let policy = EvictionPolicy::default();
        assert_eq!(policy.max_age_hours, 24);
        assert_eq!(policy.min_quality_score, 0.3);
        assert!(policy.evict_concepts);
        assert!(policy.keep_facts);
    }

    #[test]
    fn test_eviction_policy_custom() {
        let policy = EvictionPolicy::new()
            .with_max_age(48)
            .with_min_quality(0.5)
            .with_evict_concepts(false);
        
        assert_eq!(policy.max_age_hours, 48);
        assert_eq!(policy.min_quality_score, 0.5);
        assert!(!policy.evict_concepts);
    }
}
