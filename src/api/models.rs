//! Model Management API
//!
//! Implements model hot-swapping and management as specified in SPEC.md

use crate::api::chat::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Current model status
#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    pub current_model: String,
    pub base_url: String,
    pub max_tokens: usize,
    pub embedding_model: String,
    pub embedding_dim: usize,
    pub is_healthy: bool,
}

/// Update model request
#[derive(Debug, Deserialize)]
pub struct UpdateModelRequest {
    /// New model name (e.g., "qwen3:8b", "llama3:70b")
    pub model: Option<String>,
    /// New base URL for the LLM API (e.g., "http://localhost:11434")
    pub base_url: Option<String>,
    /// Max tokens for completions (for future use)
    #[allow(dead_code)]
    pub max_tokens: Option<usize>,
    /// Custom temperature for the session (for future use)
    #[allow(dead_code)]
    pub temperature: Option<f32>,
}

/// Update model response
#[derive(Debug, Clone, Serialize)]
pub struct UpdateModelResponse {
    pub success: bool,
    pub message: String,
    pub new_model: Option<String>,
    pub new_base_url: Option<String>,
}

/// Validation result
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub model: String,
    pub base_url: String,
    pub message: String,
}

/// Get current model status
pub async fn get_model_status(
    State(state): State<Arc<AppState>>,
) -> Json<ModelStatus> {
    let agent = state.agent.read().await;
    let model_config = state.model_config.read().await;
    
    if let Some(agent) = agent.as_ref() {
        let current_model = agent.current_model_name().await;
        // Get the actual config from the agent (includes runtime updates)
        let full_config = agent.current_model_config().await;
        Json(ModelStatus {
            current_model,
            base_url: model_config.base_url.clone(),
            max_tokens: full_config.max_tokens,
            embedding_model: model_config.embedding_model.clone(),
            embedding_dim: full_config.embedding_dim,
            is_healthy: true,
        })
    } else {
        Json(ModelStatus {
            current_model: "not_initialized".to_string(),
            base_url: "not_initialized".to_string(),
            max_tokens: 0,
            embedding_model: "not_initialized".to_string(),
            embedding_dim: 0,
            is_healthy: false,
        })
    }
}

/// Update model configuration (hot-swap)
pub async fn update_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateModelRequest>,
) -> Result<Json<UpdateModelResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get current model config
    let mut model_config = state.model_config.write().await;
    let current_base_url = model_config.base_url.clone();
    let current_model = model_config.name.clone();
    
    // Determine new values
    let new_base_url = request.base_url.clone().unwrap_or_else(|| current_base_url.clone());
    let new_model = request.model.clone().unwrap_or_else(|| current_model.clone());
    
    // If base_url is provided, we need to validate the new endpoint
    if let Some(ref base_url) = request.base_url {
        if base_url != &current_base_url {
            let test_url = format!("{}/v1/models", base_url);
            
            let client = reqwest::Client::new();
            match client.get(&test_url).send().await {
                Ok(response) if response.status().is_success() => {
                    tracing::info!("Model endpoint {} is accessible", base_url);
                }
                Ok(response) => {
                    return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
                        "error": format!("Model endpoint returned error: {}", response.status()),
                        "success": false
                    }))));
                }
                Err(e) => {
                    return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
                        "error": format!("Cannot connect to model endpoint: {}", e),
                        "success": false
                    }))));
                }
            }
        }
    }
    
    // Create new model config
    let new_config = crate::config::ModelConfig {
        name: new_model.clone(),
        base_url: new_base_url.clone(),
        ..(*model_config).clone()
    };
    
    // Update model_config in AppState
    *model_config = new_config.clone();
    
    // Update the agent's LLM client
    let agent = state.agent.read().await;
    if let Some(agent) = agent.as_ref() {
        if let Err(e) = agent.update_model(new_config).await {
            tracing::error!("Failed to update agent model: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to update agent model: {}", e),
                "success": false
            }))));
        }
    }
    
    let message = format!(
        "Model hot-swapped from {}@{} to {}@{}",
        current_model, current_base_url, new_model, new_base_url
    );
    
    tracing::info!("{}", message);
    
    Ok(Json(UpdateModelResponse {
        success: true,
        message,
        new_model: Some(new_model),
        new_base_url: Some(new_base_url),
    }))
}

/// Validate model configuration
pub async fn validate_model(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<UpdateModelRequest>,
) -> Result<Json<ValidationResult>, (StatusCode, Json<serde_json::Value>)> {
    let base_url = request.base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let model = request.model.unwrap_or_else(|| "qwen3:8b".to_string());
    
    let test_url = format!("{}/v1/chat/completions", base_url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    
    let test_request = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "test"}],
        "max_tokens": 1
    });
    
    match client.post(&test_url).json(&test_request).send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(Json(ValidationResult {
                    valid: true,
                    model,
                    base_url,
                    message: "Model configuration is valid".to_string(),
                }))
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Ok(Json(ValidationResult {
                    valid: false,
                    model,
                    base_url,
                    message: format!("Model returned error {}: {}", status, body),
                }))
            }
        }
        Err(e) => {
            Ok(Json(ValidationResult {
                valid: false,
                model,
                base_url,
                message: format!("Cannot connect to model: {}", e),
            }))
        }
    }
}

/// List available models on the current endpoint
pub async fn list_available_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let base_url = state.model_config.read().await.base_url.clone();
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    
    let models_url = format!("{}/v1/models", base_url);
    
    match client.get(&models_url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<serde_json::Value>().await {
                Ok(models) => Ok(Json(models)),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                    "error": format!("Failed to parse models response: {}", e)
                })))),
            }
        }
        Ok(response) => Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": format!("Failed to get models: {}", response.status())
        })))),
        Err(e) => Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error": format!("Cannot connect to model endpoint: {}", e)
        })))),
    }
}

/// Get learned concepts from training memory
#[derive(Debug, Clone, Serialize)]
pub struct LearnedConcept {
    pub id: String,
    pub content: String,
    pub created_at: String,
    pub source: String,
    pub tags: Vec<String>,
}

/// Get learned concepts response
#[derive(Debug, Clone, Serialize)]
pub struct LearnedConceptsResponse {
    pub concepts: Vec<LearnedConcept>,
    pub total: usize,
    pub namespace: String,
}

/// Get learned concepts from training memory
pub async fn get_learned_concepts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LearnedConceptsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Query the training namespace for learned concepts
    match state.memory_store.list("training", 100) {
        Ok(memories) => {
            let concepts: Vec<LearnedConcept> = memories
                .into_iter()
                .filter(|m| {
                    // Filter to concept-type memories
                    matches!(m.memory_type, crate::models::MemoryType::Concept) || 
                    m.tags.iter().any(|t| t.contains("learned"))
                })
                .map(|m| {
                    let source = m.metadata.get("source_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    LearnedConcept {
                        id: m.id,
                        content: m.content,
                        created_at: m.created_at.to_rfc3339(),
                        source,
                        tags: m.tags.clone(),
                    }
                })
                .collect();
            
            let total = concepts.len();
            
            Ok(Json(LearnedConceptsResponse {
                concepts,
                total,
                namespace: "training".to_string(),
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to query training memory: {}", e)
        })))),
    }
}

/// Search concepts by query
#[derive(Debug, Deserialize)]
pub struct SearchConceptsRequest {
    pub query: String,
    pub limit: Option<usize>,
}

/// Search concepts response
#[derive(Debug, Clone, Serialize)]
pub struct SearchConceptsResponse {
    pub concepts: Vec<LearnedConcept>,
    pub query: String,
    pub total: usize,
}

/// Search learned concepts by semantic similarity
pub async fn search_learned_concepts(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SearchConceptsRequest>,
) -> Result<Json<SearchConceptsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let limit = request.limit.unwrap_or(10).min(50);
    
    // Generate embedding for the query
    let embedding = match state.embedding_client.embed(&request.query).await {
        Ok(e) => e,
        Err(e) => {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to generate embedding: {}", e)
            }))));
        }
    };
    
    // Query training namespace
    match state.memory_store.query(&embedding, "training", limit, 0.3) {
        Ok(results) => {
            let concepts: Vec<LearnedConcept> = results
                .into_iter()
                .map(|r| {
                    let source = r.memory.metadata.get("source_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    LearnedConcept {
                        id: r.memory.id,
                        content: r.memory.content,
                        created_at: r.memory.created_at.to_rfc3339(),
                        source,
                        tags: r.memory.tags.clone(),
                    }
                })
                .collect();
            
            let total = concepts.len();
            
            Ok(Json(SearchConceptsResponse {
                concepts,
                query: request.query,
                total,
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to search concepts: {}", e)
        })))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_status_serialization() {
        let status = ModelStatus {
            current_model: "qwen3:8b".to_string(),
            base_url: "http://localhost:11434".to_string(),
            max_tokens: 8192,
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: 768,
            is_healthy: true,
        };
        
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("qwen3:8b"));
        assert!(json.contains("healthy"));
    }

    #[test]
    fn test_update_model_request_deserialization() {
        let json = r#"{"model": "llama3:70b", "base_url": "http://ollama:11434"}"#;
        let request: UpdateModelRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.model, Some("llama3:70b".to_string()));
        assert_eq!(request.base_url, Some("http://ollama:11434".to_string()));
    }

    #[test]
    fn test_validation_result_serialization() {
        let result = ValidationResult {
            valid: true,
            model: "qwen3:8b".to_string(),
            base_url: "http://localhost:11434".to_string(),
            message: "OK".to_string(),
        };
        
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"valid\":true"));
    }

    #[test]
    fn test_learned_concept_serialization() {
        let concept = LearnedConcept {
            id: "test-id".to_string(),
            content: "Rust is a systems programming language".to_string(),
            created_at: "2026-03-28T12:00:00Z".to_string(),
            source: "https://rust.example.com".to_string(),
            tags: vec!["learned".to_string(), "programming".to_string()],
        };
        
        let json = serde_json::to_string(&concept).unwrap();
        assert!(json.contains("Rust"));
        assert!(json.contains("learned"));
    }
}
