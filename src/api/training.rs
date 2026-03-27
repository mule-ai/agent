//! Training API handlers
//! 
//! Implements the training API endpoints as specified in SPEC.md

use crate::api::chat::AppState;
use crate::models::{TrainingJob, TrainingStatus};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
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
}
