//! Configuration management for AGI Agent
//! 
//! Loads configuration from agent.toml as specified in SPEC.md

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_workers")]
    pub workers: usize,
}

fn default_workers() -> usize { 4 }

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            workers: 4,
        }
    }
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub base_url: String,
    pub name: String,
    pub embedding_model: String,
    #[serde(default = "default_embedding_dim")]
    pub embedding_dim: usize,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    pub api_key: Option<String>,
}

fn default_embedding_dim() -> usize { 768 }
fn default_max_tokens() -> usize { 8192 }

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            name: "qwen3:8b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: 768,
            max_tokens: 8192,
            api_key: None,
        }
    }
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub storage_path: PathBuf,
    #[serde(default = "default_retrieval_ttl")]
    pub retrieval_ttl_hours: u32,
    #[serde(default = "default_namespace")]
    pub default_namespace: String,
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
    #[serde(default = "default_query_limit")]
    pub query_limit: usize,
}

fn default_retrieval_ttl() -> u32 { 24 }
fn default_namespace() -> String { "retrieval".to_string() }
fn default_min_similarity() -> f32 { 0.6 }
fn default_query_limit() -> usize { 10 }

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(".agent/memory"),
            retrieval_ttl_hours: 24,
            default_namespace: "retrieval".to_string(),
            min_similarity: 0.6,
            query_limit: 10,
        }
    }
}

/// Search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub instance: String,
    #[serde(default = "default_timeout")]
    pub timeout: u32,
}

fn default_timeout() -> u32 { 30 }

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            instance: "http://localhost:8088".to_string(),
            timeout: 30,
        }
    }
}

/// Training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    #[serde(default)]
    pub enabled: bool,
    pub schedule: String,
    pub model: String,
    pub output_path: PathBuf,
    #[serde(default = "default_epochs")]
    pub epochs: usize,
    #[serde(default = "default_training_batch_size")]
    pub batch_size: usize,
    #[serde(default)]
    pub gradient_accumulation_steps: usize,
    #[serde(default = "default_training_learning_rate")]
    pub learning_rate: f32,
    #[serde(default = "default_lora_rank")]
    pub lora_rank: usize,
    #[serde(default)]
    pub warmup_steps: usize,
    #[serde(default)]
    pub lr_scheduler: String,
    #[serde(default)]
    pub early_stopping_patience: usize,
    #[serde(default)]
    pub min_loss_improvement: f32,
}

fn default_epochs() -> usize { 3 }
fn default_training_batch_size() -> usize { 4 }
fn default_training_learning_rate() -> f32 { 1e-4 }
fn default_lora_rank() -> usize { 16 }

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: "0 2 * * *".to_string(),
            model: "qwen3:8b".to_string(),
            output_path: PathBuf::from(".agent/models"),
            epochs: 3,
            batch_size: 4,
            gradient_accumulation_steps: 1,
            learning_rate: 1e-4,
            lora_rank: 16,
            warmup_steps: 10,
            lr_scheduler: "cosine".to_string(),
            early_stopping_patience: 3,
            min_loss_improvement: 0.01,
        }
    }
}

/// Online Learning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineLearningConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_online_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_max_buffer_size")]
    pub max_buffer_size: usize,
    #[serde(default = "default_replay_ratio")]
    pub replay_ratio: f32,
    #[serde(default = "default_online_learning_rate")]
    pub learning_rate: f32,
    #[serde(default = "default_min_buffer_for_training")]
    pub min_buffer_for_training: usize,
    #[serde(default)]
    pub min_learning_rate: f32,
    #[serde(default)]
    pub max_learning_rate: f32,
    #[serde(default = "default_update_interval_seconds")]
    pub update_interval_seconds: u64,
    #[serde(default)]
    pub adaptive_learning_rate: bool,
}

fn default_online_batch_size() -> usize { 16 }
fn default_max_buffer_size() -> usize { 1000 }
fn default_replay_ratio() -> f32 { 0.3 }
fn default_online_learning_rate() -> f32 { 1e-5 }
fn default_min_buffer_for_training() -> usize { 50 }
fn default_update_interval_seconds() -> u64 { 300 }

impl Default for OnlineLearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            batch_size: 16,
            max_buffer_size: 1000,
            replay_ratio: 0.3,
            learning_rate: 1e-5,
            min_buffer_for_training: 50,
            min_learning_rate: 1e-6,
            max_learning_rate: 1e-4,
            update_interval_seconds: 300,
            adaptive_learning_rate: true,
        }
    }
}

/// Summarization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: None,
            model: "gpt-4o-mini".to_string(),
        }
    }
}

/// Root application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub model: ModelConfig,
    pub memory: MemoryConfig,
    pub search: SearchConfig,
    pub training: TrainingConfig,
    #[serde(default)]
    pub online_learning: OnlineLearningConfig,
    pub summarization: SummarizationConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            model: ModelConfig::default(),
            memory: MemoryConfig::default(),
            search: SearchConfig::default(),
            training: TrainingConfig::default(),
            online_learning: OnlineLearningConfig::default(),
            summarization: SummarizationConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from("agent.toml")
    }

    pub fn load_from(path: impl AsRef<str>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let locations = [
            PathBuf::from(path),
            PathBuf::from(".").join(path),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(path),
        ];

        for location in &locations {
            if location.exists() {
                let content = std::fs::read_to_string(location)
                    .with_context(|| format!("Failed to read config from {}", location.display()))?;
                return Self::from_toml(&content);
            }
        }

        tracing::warn!("No config file found, using defaults");
        Ok(Self::default())
    }

    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        let config: AppConfig = toml::from_str(content)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML: {}", e))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.model.name, "qwen3:8b");
        assert_eq!(config.memory.default_namespace, "retrieval");
    }

    #[test]
    fn test_parse_toml() {
        let toml_content = r#"
[server]
host = "127.0.0.1"
port = 9000
workers = 8

[model]
base_url = "http://ollama:11434"
name = "llama3:70b"
embedding_model = "mxbai-embed-large"
embedding_dim = 1024

[memory]
storage_path = "/data/memory"
retrieval_ttl_hours = 48
default_namespace = "long_term"
min_similarity = 0.7
query_limit = 20

[search]
instance = "https://search.example.com"
timeout = 60

[training]
enabled = true
schedule = "0 3 * * *"
model = "llama3:70b"
output_path = "/models"
batch_size = 8
steps = 1000

[summarization]
provider = "anthropic"
model = "claude-3-haiku"
"#;
        
        let config: AppConfig = toml::from_str(toml_content).unwrap();
        
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.workers, 8);
        assert_eq!(config.model.name, "llama3:70b");
        assert_eq!(config.model.embedding_dim, 1024);
        assert_eq!(config.memory.retrieval_ttl_hours, 48);
        assert!(config.training.enabled);
    }

    #[test]
    fn test_server_config_defaults() {
        let server = ServerConfig::default();
        assert_eq!(server.host, "0.0.0.0");
        assert_eq!(server.port, 8080);
    }

    #[test]
    fn test_model_config_defaults() {
        let model = ModelConfig::default();
        assert_eq!(model.base_url, "http://localhost:11434");
        assert_eq!(model.name, "qwen3:8b");
    }

    #[test]
    fn test_memory_config_defaults() {
        let memory = MemoryConfig::default();
        assert_eq!(memory.retrieval_ttl_hours, 24);
        assert_eq!(memory.default_namespace, "retrieval");
    }
}
