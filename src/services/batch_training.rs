//! Batch Training Service
//!
//! Implements batch training using the training module components:
//! - TrainingPipeline for orchestrating training runs
//! - TrainingDataAccumulator for collecting examples from memory
//! - ModelRegistry for managing trained model versions
//!
//! This service bridges the gap between the training module and the rest of the system.

use crate::config::TrainingConfig;
use crate::memory::store::MemoryStore;
use crate::models::{TrainingExample, TrainingJob, TrainingSource};
use crate::training::{TrainingDataAccumulator, TrainingPipeline};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Batch training service status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BatchTrainingStatus {
    Idle,
    Collecting,
    Training,
    Completed,
    Failed,
}

impl Default for BatchTrainingStatus {
    fn default() -> Self {
        BatchTrainingStatus::Idle
    }
}

impl std::fmt::Display for BatchTrainingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchTrainingStatus::Idle => write!(f, "idle"),
            BatchTrainingStatus::Collecting => write!(f, "collecting"),
            BatchTrainingStatus::Training => write!(f, "training"),
            BatchTrainingStatus::Completed => write!(f, "completed"),
            BatchTrainingStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Batch training statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchTrainingStats {
    pub status: String,
    pub examples_collected: usize,
    pub models_trained: usize,
    pub last_training: Option<String>,
    pub current_job: Option<String>,
    pub total_jobs: usize,
}

/// Batch training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTrainingConfig {
    pub min_examples: usize,
    pub max_examples: usize,
    pub quality_threshold: f32,
    pub auto_trigger: bool,
}

impl Default for BatchTrainingConfig {
    fn default() -> Self {
        Self {
            min_examples: 50,
            max_examples: 10000,
            quality_threshold: 0.3,
            auto_trigger: false,
        }
    }
}

/// Batch Training Service for scheduled/batch training
pub struct BatchTrainingService {
    config: BatchTrainingConfig,
    training_config: TrainingConfig,
    /// Internal training pipeline
    pipeline: Arc<RwLock<Option<TrainingPipeline>>>,
    /// Data accumulator for collecting examples
    accumulator: Arc<RwLock<TrainingDataAccumulator>>,
    /// Memory store reference
    memory_store: Option<Arc<dyn MemoryStore>>,
    /// Statistics
    stats: Arc<RwLock<BatchTrainingStats>>,
    /// Job history
    job_history: Arc<RwLock<Vec<TrainingJob>>>,
    /// Current status
    status: Arc<RwLock<BatchTrainingStatus>>,
    /// Path to persist examples to disk
    examples_path: PathBuf,
}

impl BatchTrainingService {
    /// Create a new batch training service with default examples path
    pub fn new(training_config: TrainingConfig) -> Self {
        // Default path: ~/.agi/training/examples.jsonl
        let examples_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agi/training/examples.jsonl");
        
        let service = Self {
            config: BatchTrainingConfig::default(),
            training_config,
            pipeline: Arc::new(RwLock::new(None)),
            accumulator: Arc::new(RwLock::new(TrainingDataAccumulator::new(10000))),
            memory_store: None,
            stats: Arc::new(RwLock::new(BatchTrainingStats::default())),
            job_history: Arc::new(RwLock::new(Vec::new())),
            status: Arc::new(RwLock::new(BatchTrainingStatus::Idle)),
            examples_path,
        };
        
        // Load persisted examples on creation
        service.load_examples();
        
        service
    }

    /// Create with custom batch training config (for future use)
    #[allow(dead_code)]
    pub fn with_config(training_config: TrainingConfig, config: BatchTrainingConfig) -> Self {
        let examples_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agi/training/examples.jsonl");
        
        let service = Self {
            config,
            training_config,
            pipeline: Arc::new(RwLock::new(None)),
            accumulator: Arc::new(RwLock::new(TrainingDataAccumulator::new(10000))),
            memory_store: None,
            stats: Arc::new(RwLock::new(BatchTrainingStats::default())),
            job_history: Arc::new(RwLock::new(Vec::new())),
            status: Arc::new(RwLock::new(BatchTrainingStatus::Idle)),
            examples_path,
        };
        
        // Load persisted examples on creation
        service.load_examples();
        
        service
    }
    
    /// Create with custom examples path
    #[allow(dead_code)]
    pub fn with_examples_path(training_config: TrainingConfig, examples_path: PathBuf) -> Self {
        let service = Self {
            config: BatchTrainingConfig::default(),
            training_config,
            pipeline: Arc::new(RwLock::new(None)),
            accumulator: Arc::new(RwLock::new(TrainingDataAccumulator::new(10000))),
            memory_store: None,
            stats: Arc::new(RwLock::new(BatchTrainingStats::default())),
            job_history: Arc::new(RwLock::new(Vec::new())),
            status: Arc::new(RwLock::new(BatchTrainingStatus::Idle)),
            examples_path,
        };
        
        // Load persisted examples on creation
        service.load_examples();
        
        service
    }

    /// Load examples from disk (called on service creation)
    fn load_examples(&self) {
        if !self.examples_path.exists() {
            tracing::info!("No persisted examples found at {:?}", self.examples_path);
            return;
        }
        
        match std::fs::read_to_string(&self.examples_path) {
            Ok(content) => {
                let loaded: Vec<TrainingExample> = content
                    .lines()
                    .filter_map(|line| {
                        if line.trim().is_empty() {
                            return None;
                        }
                        serde_json::from_str(line).ok()
                    })
                    .collect();
                
                let count = loaded.len();
                if count > 0 {
                    // Use try_write to avoid blocking
                    let mut accumulator = self.accumulator.try_write();
                    if let Ok(mut acc) = accumulator {
                        for example in loaded {
                            acc.add(example);
                        }
                        tracing::info!("Loaded {} persisted examples from {:?}", count, self.examples_path);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load persisted examples: {}", e);
            }
        }
    }

    /// Save examples to disk (called after adding examples)
    async fn save_examples(&self) {
        let accumulator = self.accumulator.read().await;
        let examples = accumulator.examples();
        
        if examples.is_empty() {
            // Remove file if no examples
            if self.examples_path.exists() {
                if let Err(e) = std::fs::remove_file(&self.examples_path) {
                    tracing::warn!("Failed to remove empty examples file: {}", e);
                }
            }
            return;
        }
        
        // Create parent directory if needed
        if let Some(parent) = self.examples_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create examples directory: {}", e);
                return;
            }
        }
        
        // Write JSONL
        let jsonl: String = examples
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        
        if let Err(e) = std::fs::write(&self.examples_path, jsonl) {
            tracing::error!("Failed to save examples to {:?}: {}", self.examples_path, e);
        } else {
            tracing::debug!("Saved {} examples to {:?}", examples.len(), self.examples_path);
        }
    }

    /// Set the memory store for collecting examples (for future use)
    #[allow(dead_code)]
    pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store);
        self
    }

    /// Get current status
    pub async fn get_status(&self) -> BatchTrainingStatus {
        self.status.read().await.clone()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> BatchTrainingStats {
        let mut stats = self.stats.read().await.clone();
        // Include current status from status field
        let current_status = self.status.read().await.clone();
        stats.status = current_status.to_string();
        stats
    }

    /// Get job history (for future use)
    #[allow(dead_code)]
    pub async fn get_job_history(&self) -> Vec<TrainingJob> {
        self.job_history.read().await.clone()
    }

    /// Initialize the training pipeline
    pub async fn initialize(&self) -> Result<()> {
        let mut pipeline_guard = self.pipeline.write().await;
        
        if pipeline_guard.is_none() {
            let models_dir = self.training_config.output_path.clone();
            std::fs::create_dir_all(&models_dir)
                .context("Failed to create models directory")?;
            
            let pipeline = TrainingPipeline::new(
                self.training_config.clone(),
                models_dir,
            );
            
            *pipeline_guard = Some(pipeline);
            tracing::info!("Batch training pipeline initialized");
        }
        
        Ok(())
    }

    /// Collect training examples from memory store
    pub async fn collect_from_memory(&self) -> Result<usize> {
        let store = self.memory_store
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Memory store not set"))?;
        
        // Update status
        {
            let mut status = self.status.write().await;
            *status = BatchTrainingStatus::Collecting;
        }
        
        // Query training namespace for examples
        let memories = store.list("training", self.config.max_examples)
            .unwrap_or_default();
        
        let mut count = 0;
        
        for memory in memories {
            // Skip low quality examples
            if let Some(quality) = memory.metadata.get("quality_score")
                .and_then(|v| v.as_f64())
            {
                if (quality as f32) < self.config.quality_threshold {
                    continue;
                }
            }
            
            // Convert memory to training example
            let example = self.memory_to_example(&memory);
            
            if let Some(ex) = example {
                let mut accumulator = self.accumulator.write().await;
                accumulator.add(ex);
                count += 1;
            }
        }
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.examples_collected = count;
        }
        
        tracing::info!("Collected {} training examples from memory", count);
        
        Ok(count)
    }

    /// Convert a memory to a training example
    fn memory_to_example(&self, memory: &crate::models::Memory) -> Option<TrainingExample> {
        // Extract prompt/completion from memory content
        // Format expected: "Q: ...\nA: ..." or similar structured format
        
        let content = &memory.content;
        
        // Try to parse structured content
        let (prompt, completion) = if content.contains("Q:") && content.contains("A:") {
            let parts: Vec<&str> = content.split("Q:").collect();
            if parts.len() > 1 {
                let qa_parts: Vec<&str> = parts[1].split("A:").collect();
                if qa_parts.len() > 1 {
                    (
                        qa_parts[0].trim().to_string(),
                        qa_parts[1].trim().to_string(),
                    )
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            // For non-structured content, use as both prompt and completion
            // with the content being the completion
            (format!("General: {}", &content[..content.len().min(100)]), content.clone())
        };
        
        let quality_score = memory.metadata.get("quality_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;
        
        Some(TrainingExample {
            id: memory.id.clone(),
            prompt,
            completion,
            reasoning: String::new(),
            reward: quality_score,
            source: TrainingSource::Memory,
            created_at: memory.created_at,
            quality_score,
            used_in_training: false,
        })
    }

    /// Add a training example directly
    pub async fn add_example(&self, example: TrainingExample) {
        let mut accumulator = self.accumulator.write().await;
        accumulator.add(example);
        
        let mut stats = self.stats.write().await;
        stats.examples_collected += 1;
        
        // Persist to disk
        drop(accumulator);
        drop(stats);
        self.save_examples().await;
    }

    /// Get the number of accumulated examples
    pub async fn example_count(&self) -> usize {
        let accumulator = self.accumulator.read().await;
        accumulator.examples().len()
    }

    /// Check if ready for training
    pub async fn is_ready(&self) -> bool {
        self.example_count().await >= self.config.min_examples
    }

    /// Run batch training
    pub async fn train(&self) -> Result<TrainingJob> {
        // Initialize if needed
        self.initialize().await?;
        
        // Update status
        {
            let mut status = self.status.write().await;
            *status = BatchTrainingStatus::Training;
        }
        
        // Get the pipeline
        let pipeline_guard = self.pipeline.read().await;
        let pipeline = pipeline_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pipeline not initialized"))?;
        
        // Add accumulated examples
        {
            let accumulator = self.accumulator.read().await;
            let examples: Vec<_> = accumulator.examples().to_vec();
            pipeline.add_examples(examples).await;
        }
        
        // Run training
        let result = pipeline.train().await;
        
        // Update stats and status
        {
            let mut stats = self.stats.write().await;
            
            match &result {
                Ok(_job) => {
                    stats.status = "completed".to_string();
                    stats.last_training = Some(Utc::now().to_rfc3339());
                    stats.models_trained += 1;
                    stats.total_jobs += 1;
                    
                    let mut status = self.status.write().await;
                    *status = BatchTrainingStatus::Completed;
                    
                    // Clear persisted examples after successful training
                    drop(stats);
                    self.clear().await;
                }
                Err(_) => {
                    stats.status = "failed".to_string();
                    
                    let mut status = self.status.write().await;
                    *status = BatchTrainingStatus::Failed;
                }
            }
        }
        
        result
    }

    /// Get the current training job (for future use)
    #[allow(dead_code)]
    pub async fn get_current_job(&self) -> Option<TrainingJob> {
        let pipeline_guard = self.pipeline.read().await;
        if let Some(pipeline) = pipeline_guard.as_ref() {
            pipeline.get_current_job().await
        } else {
            None
        }
    }

    /// Filter examples by quality threshold and return them
    pub async fn filter_by_quality(&self, threshold: f32) -> Vec<TrainingExample> {
        let accumulator = self.accumulator.read().await;
        accumulator.filter_by_quality(threshold)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Export filtered examples to JSONL
    pub async fn export_filtered_jsonl(&self, threshold: f32) -> String {
        let examples = self.filter_by_quality(threshold).await;
        examples
            .iter()
            .map(|e| {
                serde_json::json!({
                    "prompt": e.prompt,
                    "completion": e.completion,
                    "reasoning": e.reasoning,
                })
            })
            .map(|j| serde_json::to_string(&j).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get list of trained models from the model registry
    pub async fn list_trained_models(&self) -> Vec<crate::training::ModelInfo> {
        let pipeline_guard = self.pipeline.read().await;
        if let Some(pipeline) = pipeline_guard.as_ref() {
            pipeline.list_models().await
        } else {
            Vec::new()
        }
    }

    /// Get the current active model ID
    pub async fn get_current_model(&self) -> Option<String> {
        let pipeline_guard = self.pipeline.read().await;
        if let Some(pipeline) = pipeline_guard.as_ref() {
            pipeline.get_current_model().await
        } else {
            None
        }
    }

    /// Set the current active model ID
    pub async fn set_current_model(&self, model_id: String) {
        let pipeline_guard = self.pipeline.read().await;
        if let Some(pipeline) = pipeline_guard.as_ref() {
            pipeline.set_current_model(model_id).await;
        }
    }

    /// Export accumulated examples to JSONL
    pub async fn export_jsonl(&self) -> String {
        let accumulator = self.accumulator.read().await;
        accumulator.export_jsonl()
    }

    /// Clear accumulated examples
    pub async fn clear(&self) {
        let mut accumulator = self.accumulator.write().await;
        accumulator.clear();
        
        let mut stats = self.stats.write().await;
        stats.examples_collected = 0;
        
        // Also remove persisted file
        drop(accumulator);
        drop(stats);
        if self.examples_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.examples_path) {
                tracing::warn!("Failed to remove examples file: {}", e);
            }
        }
    }

    /// Reset to idle status (for future use)
    #[allow(dead_code)]
    pub async fn reset(&self) {
        let mut status = self.status.write().await;
        *status = BatchTrainingStatus::Idle;
    }
}

impl Clone for BatchTrainingService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            training_config: self.training_config.clone(),
            pipeline: self.pipeline.clone(),
            accumulator: self.accumulator.clone(),
            memory_store: self.memory_store.clone(),
            stats: self.stats.clone(),
            job_history: self.job_history.clone(),
            status: self.status.clone(),
            examples_path: self.examples_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_display() {
        assert_eq!(BatchTrainingStatus::Idle.to_string(), "idle");
        assert_eq!(BatchTrainingStatus::Training.to_string(), "training");
        assert_eq!(BatchTrainingStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_default_config() {
        let config = BatchTrainingConfig::default();
        assert_eq!(config.min_examples, 50);
        assert_eq!(config.max_examples, 10000);
        assert_eq!(config.quality_threshold, 0.3);
    }

    #[tokio::test]
    async fn test_service_creation() {
        use crate::config::TrainingConfig;
        
        let training_config = TrainingConfig::default();
        let service = BatchTrainingService::new(training_config);
        
        assert_eq!(service.get_status().await, BatchTrainingStatus::Idle);
    }

    #[tokio::test]
    async fn test_add_example() {
        use crate::config::TrainingConfig;
        use crate::models::TrainingExample;
        
        let training_config = TrainingConfig::default();
        let service = BatchTrainingService::with_examples_path(
            training_config,
            std::env::temp_dir().join("agi_test_examples_1.jsonl"),
        );
        
        // Clear any existing data
        service.clear().await;
        
        let example = TrainingExample::new("Test prompt".to_string(), "Test completion".to_string());
        service.add_example(example).await;
        
        assert_eq!(service.example_count().await, 1);
    }

    #[tokio::test]
    async fn test_export_jsonl() {
        use crate::config::TrainingConfig;
        use crate::models::TrainingExample;
        
        let training_config = TrainingConfig::default();
        let service = BatchTrainingService::new(training_config);
        
        let example = TrainingExample::new("Test prompt".to_string(), "Test completion".to_string());
        service.add_example(example).await;
        
        let jsonl = service.export_jsonl().await;
        assert!(jsonl.contains("Test prompt"));
        assert!(jsonl.contains("Test completion"));
    }

    #[tokio::test]
    async fn test_clear() {
        use crate::config::TrainingConfig;
        use crate::models::TrainingExample;
        
        let training_config = TrainingConfig::default();
        let service = BatchTrainingService::with_examples_path(
            training_config,
            std::env::temp_dir().join("agi_test_examples_2.jsonl"),
        );
        
        // Clear any existing data first
        service.clear().await;
        
        let example = TrainingExample::new("Test prompt".to_string(), "Test completion".to_string());
        service.add_example(example).await;
        
        assert_eq!(service.example_count().await, 1);
        
        service.clear().await;
        
        assert_eq!(service.example_count().await, 0);
    }

    #[tokio::test]
    async fn test_status_transitions() {
        use crate::config::TrainingConfig;
        
        let training_config = TrainingConfig::default();
        let service = BatchTrainingService::new(training_config);
        
        assert_eq!(service.get_status().await, BatchTrainingStatus::Idle);
        
        service.reset().await;
        assert_eq!(service.get_status().await, BatchTrainingStatus::Idle);
    }

    #[tokio::test]
    async fn test_get_stats() {
        use crate::config::TrainingConfig;
        use crate::models::TrainingExample;
        
        let training_config = TrainingConfig::default();
        let service = BatchTrainingService::new(training_config);
        
        let example = TrainingExample::new("Test prompt".to_string(), "Test completion".to_string());
        service.add_example(example).await;
        
        let stats = service.get_stats().await;
        assert_eq!(stats.examples_collected, 1);
        // Status should now reflect the actual status ("idle")
        assert_eq!(stats.status, "idle");
    }
}
