//! Memory API handlers
//! 
//! Implements the memory API endpoints as specified in SPEC.md

use crate::api::chat::AppState;
use crate::models::{Memory, MemoryType};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub query: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
}

fn default_namespace() -> String { "retrieval".to_string() }
fn default_limit() -> usize { 10 }
fn default_min_similarity() -> f32 { 0.6 }

#[derive(Debug, Deserialize)]
pub struct StoreRequest {
    pub content: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub evict_to_training: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub results: Vec<QueryResultItem>,
    pub query: String,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct QueryResultItem {
    pub memory: MemoryResponse,
    pub score: f32,
}

#[derive(Debug, Serialize)]
pub struct MemoryResponse {
    pub id: String,
    pub content: String,
    pub namespace: String,
    pub tags: Vec<String>,
    pub memory_type: String,
    pub created_at: String,
}

impl From<&Memory> for MemoryResponse {
    fn from(memory: &Memory) -> Self {
        Self {
            id: memory.id.clone(),
            content: memory.content.clone(),
            namespace: memory.namespace.clone(),
            tags: memory.tags.clone(),
            memory_type: match memory.memory_type {
                MemoryType::Fact => "fact".to_string(),
                MemoryType::Concept => "concept".to_string(),
                MemoryType::Conversation => "conversation".to_string(),
                MemoryType::ToolResult => "tool_result".to_string(),
            },
            created_at: memory.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub memories: Vec<MemoryResponse>,
    pub total: usize,
    pub namespace: String,
}

#[derive(Debug, Serialize)]
pub struct StoreResponse {
    pub memory: MemoryResponse,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total: usize,
    pub by_namespace: Vec<NamespaceCount>,
    pub by_type: Vec<TypeCount>,
}

#[derive(Debug, Serialize)]
pub struct NamespaceCount {
    pub namespace: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct TypeCount {
    #[serde(rename = "type")]
    pub memory_type: String,
    pub count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub namespace: Option<String>,
    pub limit: Option<usize>,
}

// ============================================================================
// Handlers
// ============================================================================

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

/// POST /memories/query - Query memories
pub async fn query_memory(
    State(state): State<Arc<AppState>>,
    Json(request): Json<QueryRequest>,
) -> ApiResult<QueryResponse> {
    let embedding = state.embedding_client.embed(&request.query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to generate embedding: {}", e)
        }))))?;

    let results = state.memory_store.query(&embedding, &request.namespace, request.limit, request.min_similarity)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Query failed: {}", e)
        }))))?;

    let total = results.len();
    Ok(Json(QueryResponse {
        results: results
            .into_iter()
            .map(|r| QueryResultItem {
                memory: MemoryResponse::from(&r.memory),
                score: r.score,
            })
            .collect(),
        query: request.query,
        total,
    }))
}

/// POST /memories - Store a new memory
pub async fn store_memory(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StoreRequest>,
) -> ApiResult<StoreResponse> {
    let embedding = state.embedding_client.embed(&request.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to generate embedding: {}", e)
        }))))?;

    let memory_type = request.memory_type.as_ref().map(|t| match t.as_str() {
        "concept" => MemoryType::Concept,
        "conversation" => MemoryType::Conversation,
        "tool_result" => MemoryType::ToolResult,
        _ => MemoryType::Fact,
    }).unwrap_or(MemoryType::Fact);

    let mut memory = Memory::with_params(
        request.content,
        request.namespace,
        request.tags,
        Some(memory_type),
        request.evict_to_training.unwrap_or(false),
    );
    memory.embedding = embedding;

    state.memory_store.store(&memory)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to store memory: {}", e)
        }))))?;

    Ok(Json(StoreResponse {
        memory: MemoryResponse::from(&memory),
        message: "Memory stored successfully".to_string(),
    }))
}

/// GET /memories - List memories
pub async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> ApiResult<ListResponse> {
    let namespace = params.namespace.unwrap_or_else(|| "retrieval".to_string());
    let limit = params.limit.unwrap_or(100).min(1000);

    let memories = state.memory_store.list(&namespace, limit)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to list memories: {}", e)
        }))))?;

    let total = memories.len();
    Ok(Json(ListResponse {
        memories: memories.iter().map(MemoryResponse::from).collect(),
        total,
        namespace,
    }))
}

/// DELETE /memories/:id - Delete a memory
pub async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<DeleteResponse> {
    state.memory_store.delete(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to delete memory: {}", e)
        }))))?;

    Ok(Json(DeleteResponse {
        deleted: true,
        id,
    }))
}

/// GET /memories/stats - Get memory statistics
pub async fn memory_stats(State(state): State<Arc<AppState>>) -> ApiResult<StatsResponse> {
    let stats = state.memory_store.stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to get stats: {}", e)
        }))))?;

    Ok(Json(StatsResponse {
        total: stats.total,
        by_namespace: stats.by_namespace
            .into_iter()
            .map(|(namespace, count)| NamespaceCount { namespace, count })
            .collect(),
        by_type: stats.by_type
            .into_iter()
            .map(|(memory_type, count)| TypeCount { memory_type, count })
            .collect(),
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_request_defaults() {
        let json = r#"{"query": "test query"}"#;
        let request: QueryRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.query, "test query");
        assert_eq!(request.namespace, "retrieval");
        assert_eq!(request.limit, 10);
    }

    #[test]
    fn test_store_request_parsing() {
        let json = r#"{
            "content": "Test memory",
            "namespace": "training",
            "tags": ["test", "important"],
            "memory_type": "concept"
        }"#;

        let request: StoreRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.content, "Test memory");
        assert_eq!(request.namespace, "training");
    }

    #[test]
    fn test_memory_response_from_memory() {
        let memory = Memory::with_params(
            "Test content".to_string(),
            "retrieval".to_string(),
            vec!["tag1".to_string()],
            Some(MemoryType::Concept),
            false,
        );

        let response = MemoryResponse::from(&memory);
        
        assert_eq!(response.content, "Test content");
        assert_eq!(response.namespace, "retrieval");
        assert_eq!(response.memory_type, "concept");
    }
}
