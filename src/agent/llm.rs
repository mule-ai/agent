//! LLM client for communicating with llama.cpp OpenAI-compatible API
//! 
//! Calls llama-server for chat completions

use crate::config::ModelConfig;
use crate::models::Message;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// LLM client for llama.cpp API
pub struct LlmClient {
    config: ModelConfig,
    client: Client,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Send a chat request to the LLM
    pub async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| ApiMessage {
                role: match m.role {
                    crate::models::Role::System => "system",
                    crate::models::Role::User => "user",
                    crate::models::Role::Assistant => "assistant",
                }
                .to_string(),
                content: m.content.clone(),
            })
            .collect();

        let request = ChatRequest {
            model: self.config.name.clone(),
            messages: api_messages,
            stream: false,
        };

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to LLM")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error: {} - {}", status, error);
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("Failed to parse LLM response")?;

        Ok(chat_response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default())
    }

    /// Generate embedding for text
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // llama.cpp doesn't have a built-in embedding endpoint
        // For now, return a simple hash-based embedding
        // TODO: Use a dedicated embedding service
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();
        
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
        
        Ok(embedding)
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ApiMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_client_config() {
        let config = ModelConfig {
            base_url: "http://localhost:8081".to_string(),
            name: "qwen3.5-4b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: 768,
            max_tokens: 4096,
            api_key: None,
        };

        let client = LlmClient::new(config);
        assert_eq!(client.config.name, "qwen3.5-4b");
        assert!(client.config.base_url.contains("8081"));
    }
}
