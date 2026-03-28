//! Training pipeline for AGI Agent
//!
//! Implements GRPO training with the following components:
//! - Training data accumulation from memory
//! - GRPO reward functions
//! - Model training via Python/unshloth
//! - Model registry for versioning

use crate::config::TrainingConfig;
use crate::models::{TrainingExample, TrainingJob};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// GRPO reward functions as specified in SPEC.md
pub mod grpo {
    /// Reward for correct format (XML tags)
    pub fn format_reward(completion: &str) -> f32 {
        let mut score = 0.0;
        if completion.contains("<REASONING>") && completion.contains("</REASONING>") {
            score += 1.0;
        }
        if completion.contains("<SOLUTION>") && completion.contains("</SOLUTION>") {
            score += 1.0;
        }
        if completion.contains("<answer>") && completion.contains("</answer>") {
            score += 1.0;
        }
        score
    }

    /// Reward for helpful responses
    pub fn helpfulness_reward(completion: &str) -> f32 {
        let mut score = 0.0;

        // Length-based (reasonable response length)
        let len = completion.len();
        if len > 50 && len < 2000 {
            score += 0.5;
        } else if len >= 2000 {
            score += 0.3;
        }

        // Has structured content
        if completion.contains('\n') {
            score += 0.25;
        }

        // Not empty
        if !completion.trim().is_empty() {
            score += 0.25;
        }

        score
    }

    /// Combined reward function
    pub fn combined_reward(completion: &str) -> f32 {
        let format = format_reward(completion);
        let helpful = helpfulness_reward(completion);
        (format * 0.4 + helpful * 0.6).min(1.0)
    }
}

/// Training data accumulator
pub struct TrainingDataAccumulator {
    examples: Vec<TrainingExample>,
    max_examples: usize,
}

impl TrainingDataAccumulator {
    pub fn new(max_examples: usize) -> Self {
        Self {
            examples: Vec::new(),
            max_examples,
        }
    }

    /// Add a training example
    pub fn add(&mut self, example: TrainingExample) {
        if self.examples.len() >= self.max_examples {
            // Find the lowest quality example
            if let Some(min_idx) = self.examples
                .iter()
                .position(|e| e.quality_score < example.quality_score)
            {
                self.examples.remove(min_idx);
                self.examples.push(example);
            }
            // If no example has lower quality, don't add this one
        } else {
            self.examples.push(example);
        }
    }

    /// Get all training examples
    pub fn examples(&self) -> &[TrainingExample] {
        &self.examples
    }

    /// Filter by quality threshold
    pub fn filter_by_quality(&self, threshold: f32) -> Vec<&TrainingExample> {
        self.examples
            .iter()
            .filter(|e| e.quality_score >= threshold)
            .collect()
    }

    /// Clear all examples
    pub fn clear(&mut self) {
        self.examples.clear();
    }

    /// Export to JSONL format for training
    pub fn export_jsonl(&self) -> String {
        self.examples
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
}

/// Model registry for versioned model management
pub struct ModelRegistry {
    models_dir: PathBuf,
    current_model: RwLock<Option<String>>,
}

impl ModelRegistry {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            current_model: RwLock::new(None),
        }
    }

    /// Get the current active model
    pub async fn get_current_model(&self) -> Option<String> {
        self.current_model.read().await.clone()
    }

    /// Set the current active model
    pub async fn set_current_model(&self, model_id: String) {
        let mut current = self.current_model.write().await;
        *current = Some(model_id);
    }

    /// List all trained models
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        let mut models = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.models_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        let model_id = entry.file_name().to_string_lossy().to_string();
                        let config_path = entry.path().join("config.json");

                        let created_at = metadata
                            .created()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
                            .flatten();

                        let metrics = if config_path.exists() {
                            std::fs::read_to_string(&config_path)
                                .ok()
                                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                                .and_then(|v| v.get("metrics").cloned())
                        } else {
                            None
                        };

                        models.push(ModelInfo {
                            model_id,
                            path: entry.path(),
                            created_at,
                            metrics,
                        });
                    }
                }
            }
        }

        models.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        models
    }

    /// Save a new model version
    pub async fn save_model(&self, model_id: String, metrics: serde_json::Value, output_dir: &Path) -> Result<PathBuf> {
        // The Python script saves directly to output_dir
        // We just need to add the config.json to record the model info
        let model_dir = output_dir.to_path_buf();
        
        // Ensure directory exists
        std::fs::create_dir_all(&model_dir)?;

        let config = serde_json::json!({
            "model_id": model_id,
            "metrics": metrics,
        });

        std::fs::write(
            model_dir.join("config.json"),
            serde_json::to_string_pretty(&config)?,
        )?;

        Ok(model_dir)
    }
}

/// Model information
#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct ModelInfo {
    pub model_id: String,
    pub path: PathBuf,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
}

/// Training pipeline orchestrator
pub struct TrainingPipeline {
    config: TrainingConfig,
    data_accumulator: RwLock<TrainingDataAccumulator>,
    model_registry: Arc<ModelRegistry>,
    current_job: RwLock<Option<TrainingJob>>,
}

impl TrainingPipeline {
    pub fn new(config: TrainingConfig, models_dir: PathBuf) -> Self {
        Self {
            data_accumulator: RwLock::new(TrainingDataAccumulator::new(10000)),
            model_registry: Arc::new(ModelRegistry::new(models_dir)),
            config,
            current_job: RwLock::new(None),
        }
    }

    /// Get current training job (for future use)
    #[allow(dead_code)]
    pub async fn get_current_job(&self) -> Option<TrainingJob> {
        self.current_job.read().await.clone()
    }

    /// Get the current active model ID
    pub async fn get_current_model(&self) -> Option<String> {
        self.model_registry.get_current_model().await
    }

    /// Set the current active model ID
    pub async fn set_current_model(&self, model_id: String) {
        self.model_registry.set_current_model(model_id).await;
    }

    /// List all trained models
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        self.model_registry.list_models().await
    }

    /// Add training examples
    pub async fn add_examples(&self, examples: Vec<TrainingExample>) {
        let mut accumulator = self.data_accumulator.write().await;
        for example in examples {
            accumulator.add(example);
        }
    }

    /// Start training
    pub async fn train(&self) -> Result<TrainingJob> {
        let mut job = TrainingJob::new(self.config.epochs, self.config.batch_size * 100);
        job.start();

        // Store job
        {
            let mut current = self.current_job.write().await;
            *current = Some(job.clone());
        }

        // Run training in background
        let config = self.config.clone();
        let registry = self.model_registry.clone();
        let accumulator = self.data_accumulator.read().await;

        let examples_jsonl = accumulator.export_jsonl();
        drop(accumulator);

        // Write training data to home directory (tmp might not exist in container)
        let home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/administrator"));
        let training_dir = home_dir.join(".agi/training");
        std::fs::create_dir_all(&training_dir)?;
        let training_data_path = training_dir.join("training_data.jsonl");
        std::fs::write(&training_data_path, &examples_jsonl)?;

        // Run Python training script using the venv python if available
        let python_cmd = if PathBuf::from("/home/administrator/.agi-venv/bin/python").exists() {
            "/home/administrator/.agi-venv/bin/python"
        } else {
            "python3"
        };

        // Run the training script
        let output_path = config.output_path.clone();
        let result = Self::run_training_script(python_cmd, &training_data_path, &output_path);

        let mut job = self.current_job.write().await;
        if let Some(ref mut j) = *job {
            match result {
                Ok(metrics) => {
                    j.complete();

                    // Save model - use a model_id based on the output directory name
                    let model_id = output_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("model")
                        .to_string();
                    registry.save_model(model_id, metrics, &output_path).await?;
                }
                Err(e) => {
                    j.fail(format!("Training failed: {}", e));
                }
            }
        }

        Ok(job.clone().unwrap_or_else(default_job))
    }

    /// Run the external training script
    fn run_training_script(python_cmd: &str, data_path: &Path, output_path: &Path) -> Result<serde_json::Value, String> {
        // Get the directory of the current binary or project
        let script_path = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(&p).join("training_script.py"))
            .unwrap_or_else(|_| PathBuf::from("training_script.py"));
        
        // Fallback to project directory if script doesn't exist next to binary
        let script_path = if script_path.exists() {
            script_path
        } else {
            PathBuf::from("/data/jbutler/mule/agent/training_script.py")
        };
        
        let data_path_str = data_path.display().to_string();
        let output_path_str = output_path.display().to_string();
        
        tracing::info!("Running training script: {} {} {} {}", python_cmd, script_path.display(), data_path_str, output_path_str);
        
        let output = std::process::Command::new(python_cmd)
            .arg(&script_path)
            .arg(&data_path_str)
            .arg(&output_path_str)
            .output()
            .map_err(|e| format!("Failed to run training script: {}", e))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        tracing::info!("Training script stdout: {}", stdout);
        if !stderr.is_empty() {
            tracing::info!("Training script stderr: {}", stderr);
        }
        
        if output.status.success() {
            // Look for the last line that is valid JSON
            let mut result = serde_json::json!({"status": "completed"});
            for line in stdout.lines().rev() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                    result = parsed;
                    break;
                }
            }
            Ok(result)
        } else {
            Err(format!("Training failed with status {:?}: stdout={} stderr={}", output.status, stdout, stderr))
        }
    }
}

/// Create a default training job for error handling
fn default_job() -> TrainingJob {
    TrainingJob::new(0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_reward() {
        assert_eq!(grpo::format_reward(""), 0.0);
        assert_eq!(grpo::format_reward("<REASONING>test</REASONING>"), 1.0);
        assert_eq!(
            grpo::format_reward("<REASONING>test</REASONING><SOLUTION>answer</SOLUTION>"),
            2.0
        );
    }

    #[test]
    fn test_helpfulness_reward() {
        assert_eq!(grpo::helpfulness_reward(""), 0.0);
        assert!(grpo::helpfulness_reward(&"a".repeat(100)) > 0.0);
    }

    #[test]
    fn test_combined_reward() {
        let reward = grpo::combined_reward("<REASONING>test</REASONING>\n\nSome helpful response");
        assert!(reward > 0.0);
    }

    #[tokio::test]
    async fn test_training_accumulator() {
        let mut accumulator = TrainingDataAccumulator::new(3);

        accumulator.add(TrainingExample::new("p1".to_string(), "c1".to_string()));
        accumulator.add(TrainingExample::new("p2".to_string(), "c2".to_string()));
        accumulator.add(TrainingExample::new("p3".to_string(), "c3".to_string()));

        assert_eq!(accumulator.examples().len(), 3);

        // Adding another should trigger replacement logic
        accumulator.add(TrainingExample::new("p4".to_string(), "c4".to_string()));
        assert!(accumulator.examples().len() <= 3);
    }

    #[test]
    fn test_export_jsonl() {
        let mut accumulator = TrainingDataAccumulator::new(10);
        accumulator.add(TrainingExample::new("prompt".to_string(), "completion".to_string()));

        let jsonl = accumulator.export_jsonl();
        assert!(jsonl.contains("prompt"));
        assert!(jsonl.contains("completion"));
    }
}
