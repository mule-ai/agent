//! Embedding client for generating text embeddings
//! 
//! Uses Ollama API for embedding generation as specified in SPEC.md

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use parking_lot::RwLock;
use lru::LruCache;

/// Configuration for embedding client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingClientConfig {
    pub base_url: String,
    pub model: String,
    pub dimensions: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    pub api_key: Option<String>,
}

fn default_batch_size() -> usize {
    32
}

impl Default for EmbeddingClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "nomic-embed-text".to_string(),
            dimensions: 768,
            batch_size: 32,
            api_key: None,
        }
    }
}

/// Response from Ollama embeddings API
#[derive(Debug, Deserialize)]
pub struct OllamaEmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

/// Embedding client for generating text embeddings
pub struct EmbeddingClient {
    client: Client,
    config: EmbeddingClientConfig,
    cache: Arc<RwLock<LruCache<String, Vec<f32>>>>,
}

impl EmbeddingClient {
    /// Create a new embedding client
    pub fn new(config: EmbeddingClientConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            cache: Arc::new(RwLock::new(LruCache::new(std::num::NonZeroUsize::new(1024).unwrap()))),
        }
    }
}

impl Default for EmbeddingClient {
    fn default() -> Self {
        Self::new(EmbeddingClientConfig::default())
    }
}

impl EmbeddingClient {
    /// Generate embedding for a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache first (use write lock since LRU get() may modify)
        {
            let mut cache = self.cache.write();
            if let Some(embedding) = cache.get(text).cloned() {
                return Ok(embedding);
            }
        }

        // Generate new embedding
        let embedding = self.generate_embedding(text).await?;

        // Cache the result
        {
            let mut cache = self.cache.write();
            cache.put(text.to_string(), embedding.clone());
        }

        Ok(embedding)
    }

    /// Generate embedding from Ollama API
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.config.base_url);

        #[derive(Serialize)]
        struct Request {
            model: String,
            prompt: String,
        }

        let response = self.client
            .post(&url)
            .json(&Request {
                model: self.config.model.clone(),
                prompt: text.to_string(),
            })
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            anyhow::bail!("Embedding API returned error: {}", response.status());
        }

        let body: OllamaEmbedResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        body.embeddings
            .into_iter()
            .next()
            .context("No embeddings in response")
    }

    /// Generate embeddings for multiple texts (for future use)
    #[allow(dead_code)]
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        
        for text in texts {
            let embedding = self.embed(text).await?;
            results.push(embedding);
        }

        Ok(results)
    }

    /// Clear the embedding cache (for future use)
    #[allow(dead_code)]
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get cache size (for future use)
    #[allow(dead_code)]
    pub fn cache_size(&self) -> usize {
        let cache = self.cache.read();
        cache.len()
    }

    /// Compute cosine similarity between two vectors (for future use)
    #[allow(dead_code)]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        dot_product / (magnitude_a * magnitude_b)
    }

    /// Compute euclidean distance between two vectors (for future use)
    #[allow(dead_code)]
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::MAX;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

/// Mock embedding client for testing without Ollama
#[allow(dead_code)]
pub struct MockEmbeddingClient {
    dimension: usize,
}

impl MockEmbeddingClient {
    #[allow(dead_code)]
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Generate a mock embedding (random or hash-based)
    #[allow(dead_code)]
    pub fn embed(&self, text: &str) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Generate deterministic "random" values from hash
        let mut embedding = Vec::with_capacity(self.dimension);
        let mut state = hash;
        for _ in 0..self.dimension {
            // Simple LCG
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

impl Default for MockEmbeddingClient {
    fn default() -> Self {
        Self::new(768)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((EmbeddingClient::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((EmbeddingClient::cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((EmbeddingClient::cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        assert!((EmbeddingClient::euclidean_distance(&a, &b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_mock_embedding() {
        let client = MockEmbeddingClient::new(128);
        
        let emb1 = client.embed("hello world");
        let emb2 = client.embed("hello world");
        let emb3 = client.embed("different text");
        
        assert_eq!(emb1.len(), 128);
        assert_eq!(emb2.len(), 128);
        assert_eq!(emb3.len(), 128);
        
        // Same text should produce same embedding
        assert_eq!(emb1, emb2);
        
        // Different text should produce different embedding
        assert_ne!(emb1, emb3);
        
        // Verify normalization
        let magnitude: f32 = emb1.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_mock_embedding_different_dimensions() {
        let client128 = MockEmbeddingClient::new(128);
        let client256 = MockEmbeddingClient::new(256);
        
        assert_eq!(client128.embed("test").len(), 128);
        assert_eq!(client256.embed("test").len(), 256);
    }

    #[test]
    fn test_embedding_client_config_default() {
        let config = EmbeddingClientConfig::default();
        assert_eq!(config.base_url, "http://localhost:11434");
        assert_eq!(config.dimensions, 768);
        assert_eq!(config.batch_size, 32);
    }
}
