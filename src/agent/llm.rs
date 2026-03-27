//! LLM client for communicating with Ollama/OpenAI-compatible APIs
//! 
//! Implements LLM client as specified in SPEC.md

use crate::config::ModelConfig;
use crate::models::Message;
use anyhow::Result;
use std::sync::Arc;
use parking_lot::RwLock;
use lru::LruCache;

/// LLM client for Ollama/OpenAI API
pub struct LlmClient {
    config: ModelConfig,
    /// Cache for recent responses (simple implementation)
    cache: Arc<RwLock<LruCache<String, String>>>,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(LruCache::new(std::num::NonZeroUsize::new(100).unwrap()))),
        }
    }

    /// Send a chat request to the LLM
    /// 
    /// Note: In a full implementation, this would call the actual LLM API.
    /// For now, it returns a placeholder response.
    pub async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        // Convert messages to API format
        let _message_count = messages.len();
        let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        
        // Check cache (simplified - just use last message as key)
        let cache_key = last_message.to_string();
        {
            let mut cache = self.cache.write();
            if let Some(cached) = cache.get(&cache_key).cloned() {
                return Ok(cached);
            }
        }

        // Generate response (placeholder - in production, call actual LLM)
        let response = format!(
            "I understand you said '{}'. This is a placeholder response. \
            In a full implementation, I would call the LLM at {} with model {}.",
            last_message, self.config.base_url, self.config.name
        );

        // Cache the response
        {
            let mut cache = self.cache.write();
            cache.put(cache_key, response.clone());
        }

        Ok(response)
    }

    /// Clear the response cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        let cache = self.cache.read();
        cache.len()
    }

    /// Generate embedding for text
    /// 
    /// Note: This is a placeholder. In production, call the embedding API.
    pub fn generate_embedding(&self, text: &str) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Generate deterministic "random" values from hash
        let mut embedding = Vec::with_capacity(self.config.embedding_dim);
        let mut state = hash;
        for _ in 0..self.config.embedding_dim {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            let value = ((state as u32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            embedding.push(value);
        }
        
        // Normalize
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for v in &mut embedding {
                *v /= magnitude;
            }
        }
        
        embedding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_client_config() {
        let config = ModelConfig {
            base_url: "http://localhost:11434".to_string(),
            name: "qwen3:8b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: 768,
            max_tokens: 8192,
            api_key: None,
        };

        let client = LlmClient::new(config);
        
        assert!(client.config.name == "qwen3:8b");
        assert!(client.config.base_url.contains("localhost"));
    }

    #[tokio::test]
    async fn test_chat_returns_response() {
        let config = ModelConfig::default();
        let client = LlmClient::new(config);
        
        let messages = vec![Message::user("Hello".to_string())];
        let response = client.chat(messages).await.unwrap();
        
        assert!(response.contains("Hello"));
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = ModelConfig::default();
        let client = LlmClient::new(config);
        
        // Initial cache should be empty
        assert_eq!(client.cache_size(), 0);
        
        // Clear empty cache should work
        client.clear_cache();
        assert_eq!(client.cache_size(), 0);
    }

    #[test]
    fn test_generate_embedding() {
        let config = ModelConfig {
            base_url: "http://localhost:11434".to_string(),
            name: "qwen3:8b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: 128,
            max_tokens: 8192,
            api_key: None,
        };
        
        let client = LlmClient::new(config);
        
        let emb = client.generate_embedding("test");
        assert_eq!(emb.len(), 128);
        
        // Verify normalization
        let magnitude: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_embedding_deterministic() {
        let config = ModelConfig::default();
        let client = LlmClient::new(config);
        
        let emb1 = client.generate_embedding("same text");
        let emb2 = client.generate_embedding("same text");
        
        assert_eq!(emb1, emb2);
        
        let emb3 = client.generate_embedding("different text");
        assert_ne!(emb1, emb3);
    }
}
