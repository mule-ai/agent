//! Training API handlers
//! 
//! Implements the training API endpoints as specified in SPEC.md

use crate::api::chat::AppState;
use crate::models::{TrainingJob, TrainingStatus};
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::Mutex;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TriggerRequest {
    #[serde(default = "default_steps")]
    pub steps: usize,
    #[serde(default)]
    #[allow(dead_code)]
    pub batch_size: Option<usize>,
}

fn default_steps() -> usize { 500 }

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub current_job: Option<TrainingJobResponse>,
    pub last_job: Option<TrainingJobResponse>,
    pub total_jobs: usize,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct TrainingJobResponse {
    pub id: String,
    pub status: String,
    pub epochs: usize,
    pub current_epoch: usize,
    pub current_step: usize,
    pub total_steps: usize,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

impl From<&TrainingJob> for TrainingJobResponse {
    fn from(job: &TrainingJob) -> Self {
        Self {
            id: job.id.clone(),
            status: job.status.to_string(),
            epochs: job.epochs,
            current_epoch: job.current_epoch,
            current_step: job.current_step,
            total_steps: job.total_steps,
            created_at: job.created_at.to_rfc3339(),
            started_at: job.started_at.map(|dt| dt.to_rfc3339()),
            completed_at: job.completed_at.map(|dt| dt.to_rfc3339()),
            error: job.error.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TriggerResponse {
    pub job_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub trained_at: String,
    pub steps: usize,
    pub loss: Option<f32>,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsListResponse {
    pub models: Vec<ModelInfo>,
}

struct TrainingState {
    current_job: Option<TrainingJob>,
    history: Vec<TrainingJob>,
    models: Vec<ModelInfo>,
}

impl Default for TrainingState {
    fn default() -> Self {
        Self {
            current_job: None,
            history: Vec::new(),
            models: Vec::new(),
        }
    }
}

lazy_static::lazy_static! {
    static ref TRAINING_STATE: Mutex<TrainingState> = Mutex::new(TrainingState::default());
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

// ============================================================================
// Handlers
// ============================================================================

/// POST /training/trigger - Trigger a training job
pub async fn trigger_training(
    _state: State<Arc<AppState>>,
    Json(request): Json<TriggerRequest>,
) -> ApiResult<TriggerResponse> {
    let mut state = TRAINING_STATE.lock().unwrap();

    if let Some(ref job) = state.current_job {
        if job.status == TrainingStatus::Training {
            return Err((StatusCode::CONFLICT, Json(serde_json::json!({
                "error": "Training already in progress",
                "job_id": job.id
            }))));
        }
    }

    let steps = if request.steps == 0 { 500 } else { request.steps };
    let mut job = TrainingJob::new(3, steps);
    let job_id = job.id.clone();

    job.start();
    
    if let Some(current) = state.current_job.take() {
        state.history.push(current);
    }
    
    state.current_job = Some(job);

    Ok(Json(TriggerResponse {
        job_id,
        status: "started".to_string(),
        message: format!("Training job started with {} steps", steps),
    }))
}

/// GET /training/status - Get training status
pub async fn get_training_status(
    _state: State<Arc<AppState>>,
) -> ApiResult<StatusResponse> {
    let state = TRAINING_STATE.lock().unwrap();

    let current_job = state.current_job.as_ref().map(|j| j.into());
    let last_job = state.history.last().map(|j| j.into());

    Ok(Json(StatusResponse {
        status: state
            .current_job
            .as_ref()
            .map(|j| j.status.to_string())
            .unwrap_or_else(|| "idle".to_string()),
        current_job,
        last_job,
        total_jobs: state.history.len(),
        models: state.models.clone(),
    }))
}

/// GET /training/models - List trained models
pub async fn list_models(
    _state: State<Arc<AppState>>,
) -> ApiResult<ModelsListResponse> {
    let state = TRAINING_STATE.lock().unwrap();
    Ok(Json(ModelsListResponse {
        models: state.models.clone(),
    }))
}

/// POST /training/cancel - Cancel current training
pub async fn cancel_training(
    _state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut state = TRAINING_STATE.lock().unwrap();

    if let Some(mut job) = state.current_job.take() {
        job.fail("Cancelled by user".to_string());
        state.history.push(job);
        
        Ok(Json(serde_json::json!({
            "cancelled": true,
            "message": "Training job cancelled"
        })))
    } else {
        Ok(Json(serde_json::json!({
            "cancelled": false,
            "message": "No training job to cancel"
        })))
    }
}

// ============================================================================
// Batch Training API Endpoints
// ============================================================================

/// Batch training status response
#[derive(Debug, Serialize)]
pub struct BatchTrainingStatusResponse {
    pub status: String,
    pub examples_collected: usize,
    pub models_trained: usize,
    pub last_training: Option<String>,
    pub current_job: Option<String>,
}

/// Get batch training status
pub async fn batch_training_status(
    State(state): State<Arc<AppState>>,
) -> Json<BatchTrainingStatusResponse> {
    let service = &state.batch_training_service;
    let stats = service.get_stats().await;
    let status = service.get_status().await;
    
    Json(BatchTrainingStatusResponse {
        status: status.to_string(),
        examples_collected: stats.examples_collected,
        models_trained: stats.models_trained,
        last_training: stats.last_training,
        current_job: stats.current_job,
    })
}

/// Collect training examples from memory
#[derive(Debug, Deserialize)]
pub struct CollectExamplesRequest {
    #[serde(default)]
    #[allow(dead_code)]
    pub namespace: Option<String>,
}

/// Collect examples response
#[derive(Debug, Serialize)]
pub struct CollectExamplesResponse {
    pub success: bool,
    pub examples_collected: usize,
    pub message: String,
}

/// Collect training examples from memory
pub async fn collect_training_examples(
    State(state): State<Arc<AppState>>,
    Json(_req): Json<CollectExamplesRequest>,
) -> Json<CollectExamplesResponse> {
    let service = &state.batch_training_service;
    
    match service.collect_from_memory().await {
        Ok(count) => Json(CollectExamplesResponse {
            success: true,
            examples_collected: count,
            message: format!("Collected {} training examples from memory", count),
        }),
        Err(e) => Json(CollectExamplesResponse {
            success: false,
            examples_collected: 0,
            message: format!("Failed to collect examples: {}", e),
        }),
    }
}

/// Add training example directly
#[derive(Debug, Deserialize)]
pub struct AddTrainingExampleRequest {
    pub prompt: String,
    pub completion: String,
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// Add training example response
#[derive(Debug, Serialize)]
pub struct AddTrainingExampleResponse {
    pub success: bool,
    pub message: String,
    pub example_id: String,
}

/// Add a training example to the batch accumulator
pub async fn add_training_example(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddTrainingExampleRequest>,
) -> Json<AddTrainingExampleResponse> {
    use crate::models::TrainingExample;
    use uuid::Uuid;
    
    let service = &state.batch_training_service;
    let example_id = Uuid::new_v4().to_string();
    
    let example = TrainingExample {
        id: example_id.clone(),
        prompt: req.prompt,
        completion: req.completion,
        reasoning: req.reasoning.unwrap_or_default(),
        reward: 0.0,
        source: crate::models::TrainingSource::Manual,
        created_at: chrono::Utc::now(),
        quality_score: 0.5,
        used_in_training: false,
    };
    
    service.add_example(example).await;
    
    Json(AddTrainingExampleResponse {
        success: true,
        message: "Example added to batch accumulator".to_string(),
        example_id,
    })
}

/// Get batch accumulator stats
#[derive(Debug, Serialize)]
pub struct AccumulatorStatsResponse {
    pub example_count: usize,
    pub is_ready: bool,
}

/// Get batch accumulator statistics
pub async fn get_accumulator_stats(
    State(state): State<Arc<AppState>>,
) -> Json<AccumulatorStatsResponse> {
    let service = &state.batch_training_service;
    
    Json(AccumulatorStatsResponse {
        example_count: service.example_count().await,
        is_ready: service.is_ready().await,
    })
}

/// Run batch training
#[derive(Debug, Deserialize)]
pub struct RunBatchTrainingRequest {
    #[serde(default)]
    pub collect_first: Option<bool>,
}

/// Run batch training response
#[derive(Debug, Serialize)]
pub struct RunBatchTrainingResponse {
    pub success: bool,
    pub message: String,
    pub job_id: Option<String>,
}

/// Run batch training with accumulated examples
pub async fn run_batch_training(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunBatchTrainingRequest>,
) -> Json<RunBatchTrainingResponse> {
    let service = &state.batch_training_service;
    
    // Optionally collect from memory first
    if req.collect_first.unwrap_or(false) {
        if let Err(e) = service.collect_from_memory().await {
            return Json(RunBatchTrainingResponse {
                success: false,
                message: format!("Failed to collect examples: {}", e),
                job_id: None,
            });
        }
    }
    
    // Check if ready
    if !service.is_ready().await {
        let count = service.example_count().await;
        return Json(RunBatchTrainingResponse {
            success: false,
            message: format!("Not ready for training. Need at least 50 examples, have {}", count),
            job_id: None,
        });
    }
    
    // Run training
    match service.train().await {
        Ok(job) => Json(RunBatchTrainingResponse {
            success: true,
            message: format!("Training job {} completed", job.id),
            job_id: Some(job.id),
        }),
        Err(e) => Json(RunBatchTrainingResponse {
            success: false,
            message: format!("Training failed: {}", e),
            job_id: None,
        }),
    }
}

/// Export accumulated examples as JSONL
#[derive(Debug, Serialize)]
pub struct ExportExamplesResponse {
    pub success: bool,
    pub jsonl: String,
    pub line_count: usize,
}

/// Export accumulated examples to JSONL format
pub async fn export_training_examples(
    State(state): State<Arc<AppState>>,
) -> Json<ExportExamplesResponse> {
    let service = &state.batch_training_service;
    
    let jsonl = service.export_jsonl().await;
    let line_count = jsonl.lines().count();
    
    Json(ExportExamplesResponse {
        success: true,
        jsonl,
        line_count,
    })
}

/// Clear accumulated examples
#[derive(Debug, Serialize)]
pub struct ClearAccumulatorResponse {
    pub success: bool,
    pub message: String,
}

/// Clear the batch accumulator
pub async fn clear_accumulator(
    State(state): State<Arc<AppState>>,
) -> Json<ClearAccumulatorResponse> {
    let service = &state.batch_training_service;
    service.clear().await;
    
    Json(ClearAccumulatorResponse {
        success: true,
        message: "Accumulator cleared".to_string(),
    })
}

// ============================================================================
// Quality Filtering Endpoints
// ============================================================================

/// Filter examples by quality request
#[derive(Debug, Deserialize)]
pub struct FilterQualityRequest {
    /// Minimum quality score threshold (0.0 to 1.0)
    pub threshold: f32,
}

/// Filter examples by quality response
#[derive(Debug, Serialize)]
pub struct FilterQualityResponse {
    pub success: bool,
    pub threshold: f32,
    pub total_examples: usize,
    pub filtered_count: usize,
    pub jsonl: String,
}

/// Filter and export examples by quality threshold
pub async fn filter_examples_by_quality(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FilterQualityRequest>,
) -> Json<FilterQualityResponse> {
    let service = &state.batch_training_service;
    
    let total = service.example_count().await;
    let jsonl = service.export_filtered_jsonl(req.threshold).await;
    let filtered_count = jsonl.lines().count();
    
    Json(FilterQualityResponse {
        success: true,
        threshold: req.threshold,
        total_examples: total,
        filtered_count,
        jsonl,
    })
}

// ============================================================================
// Model Registry Endpoints
// ============================================================================

/// List trained models response
#[derive(Debug, Serialize)]
pub struct ListTrainedModelsResponse {
    pub current_model: Option<String>,
    pub models: Vec<crate::training::ModelInfo>,
}

/// Get current model and list all trained models
pub async fn list_trained_models(
    State(state): State<Arc<AppState>>,
) -> Json<ListTrainedModelsResponse> {
    let service = &state.batch_training_service;
    
    let current_model = service.get_current_model().await;
    let models = service.list_trained_models().await;
    
    Json(ListTrainedModelsResponse {
        current_model,
        models,
    })
}

/// Set current model request
#[derive(Debug, Deserialize)]
pub struct SetCurrentModelRequest {
    pub model_id: String,
}

/// Set current model response
#[derive(Debug, Serialize)]
pub struct SetCurrentModelResponse {
    pub success: bool,
    pub model_id: String,
    pub message: String,
}

/// Set the current active model
pub async fn set_current_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetCurrentModelRequest>,
) -> Json<SetCurrentModelResponse> {
    let service = &state.batch_training_service;
    let model_id = req.model_id.clone();
    
    // Verify the model exists
    let models = service.list_trained_models().await;
    let model_exists = models.iter().any(|m| m.model_id == model_id);
    
    if !model_exists {
        let available: Vec<String> = models.iter().map(|m| m.model_id.clone()).collect();
        return Json(SetCurrentModelResponse {
            success: false,
            model_id: model_id.clone(),
            message: format!("Model '{}' not found. Available models: {:?}", 
                model_id, available),
        });
    }
    
    service.set_current_model(model_id.clone()).await;
    
    Json(SetCurrentModelResponse {
        success: true,
        model_id,
        message: "Current model updated".to_string(),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_request_defaults() {
        let json = r#"{}"#;
        let request: TriggerRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.steps, 500);
        assert!(request.batch_size.is_none());
    }

    #[test]
    fn test_training_job_response_from_job() {
        let mut job = TrainingJob::new(3, 125);
        job.start();

        let response = TrainingJobResponse::from(&job);
        
        assert_eq!(response.status, "training");
        assert_eq!(response.epochs, 3);
        assert!(response.started_at.is_some());
    }

    #[test]
    fn test_training_status_display() {
        assert_eq!(TrainingStatus::Pending.to_string(), "pending");
        assert_eq!(TrainingStatus::Training.to_string(), "training");
        assert_eq!(TrainingStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_filter_quality_request() {
        let json = r#"{"threshold": 0.75}"#;
        let request: FilterQualityRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.threshold, 0.75);
    }

    #[test]
    fn test_set_current_model_request() {
        let json = r#"{"model_id": "qwen3:8b-v1"}"#;
        let request: SetCurrentModelRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.model_id, "qwen3:8b-v1");
    }
}
