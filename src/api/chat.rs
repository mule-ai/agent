//! Chat completions API handler
//! 
//! Implements the OpenAI-compatible chat API as specified in SPEC.md

use crate::models::{Message, Role};
use axum::{
    extract::{State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<RwLock<Option<crate::agent::Agent>>>,
    pub memory_store: Arc<crate::memory::SqliteMemoryStore>,
    pub embedding_client: Arc<crate::memory::EmbeddingClient>,
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Chat completion request (OpenAI-compatible)
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
}

/// Chat message (OpenAI-compatible)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn into_message(self) -> Message {
        let role = match self.role.as_str() {
            "system" => Role::System,
            "assistant" => Role::Assistant,
            _ => Role::User,
        };
        Message::new(role, self.content)
    }

    pub fn from_message(msg: &Message) -> Self {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        Self {
            role: role.to_string(),
            content: msg.content.clone(),
            name: None,
        }
    }
}

/// Chat completion response (OpenAI-compatible)
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<Model>,
}

#[derive(Debug, Serialize)]
pub struct Model {
    pub id: String,
    pub object: String,
    pub owned_by: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /v1/models - List available models
pub async fn models_handler() -> impl IntoResponse {
    Json(ModelsResponse {
        object: "list".to_string(),
        data: vec![Model {
            id: "agent".to_string(),
            object: "model".to_string(),
            owned_by: "agi-agent".to_string(),
        }],
    })
}

/// POST /v1/chat/completions - Create a chat completion
pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<serde_json::Value>)> {
    let messages: Vec<Message> = request.messages
        .into_iter()
        .map(|m| m.into_message())
        .collect();

    let agent = state.agent.read().await;
    
    let agent = agent.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error": {
                "message": "Agent not initialized",
                "type": "service_unavailable",
                "code": "agent_not_ready"
            }
        })))
    })?;

    let response = agent.chat(messages).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": {
                "message": e.to_string(),
                "type": "internal_error",
                "code": "internal_error"
            }
        })))
    })?;

    Ok(Json(ChatResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: request.model,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: response.content,
                name: None,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    }))
}

/// WebSocket handler for streaming (placeholder)
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|_socket| async {
        // Streaming implementation would go here
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_into_message() {
        let chat_msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            name: None,
        };

        let msg = chat_msg.into_message();
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_message_roundtrip() {
        let msg = Message::user("Test content".to_string());
        let chat_msg = ChatMessage::from_message(&msg);
        
        assert_eq!(chat_msg.role, "user");
        assert_eq!(chat_msg.content, "Test content");
    }

    #[test]
    fn test_chat_request_deserialization() {
        let json = r#"{
            "model": "agent",
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "stream": false,
            "temperature": 0.7
        }"#;

        let request: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, "agent");
        assert_eq!(request.messages.len(), 1);
    }
}
