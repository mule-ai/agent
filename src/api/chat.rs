//! Chat completions API handler
//!
//! Implements the OpenAI-compatible chat API as specified in SPEC.md

use crate::knowledge::{ArxivClient, WikipediaClient, WebFetcher, KnowledgeConfig};
use crate::models::{Message, Role};
use crate::services::{
    CuriosityEngine, MemoryEvictionService, SearchLearningService, SessionReviewService, 
    OnlineLearningService, SelfImproveEngine, TheoryOfMindEngine, BatchTrainingService,
    SchedulerService,
};
use crate::tools::ToolRegistry;
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
    #[allow(dead_code)]
    pub tool_registry: Arc<ToolRegistry>,
    #[allow(dead_code)]
    pub service_manager: Arc<crate::services::ServiceManager>,
    pub session_review_service: Arc<SessionReviewService>,
    pub memory_eviction_service: Arc<MemoryEvictionService>,
    pub search_learning_service: Arc<SearchLearningService>,
    pub curiosity_engine: Arc<CuriosityEngine>,
    pub online_learning_service: Arc<OnlineLearningService>,
    pub self_improve_engine: Arc<SelfImproveEngine>,
    pub theory_of_mind_engine: Arc<TheoryOfMindEngine>,
    pub batch_training_service: Arc<BatchTrainingService>,
    pub scheduler_service: Arc<SchedulerService>,
    pub session_store: Option<Arc<crate::agent::SessionStore>>,
    // External knowledge sources
    pub wikipedia: WikipediaClient,
    pub arxiv: ArxivClient,
    pub web_fetcher: WebFetcher,
    pub knowledge_config: KnowledgeConfig,
    // Dynamic model configuration for hot-swapping
    pub model_config: Arc<RwLock<crate::config::ModelConfig>>,
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
    #[allow(dead_code)]
    pub stream: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    pub temperature: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub max_tokens: Option<i32>,
}

/// Content part for multi-modal messages (API-level)
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatContentPart {
    /// Text content
    Text { text: String },
    /// Image from URL
    ImageUrl { image_url: ImageUrlContent },
    /// Input audio
    InputAudio { input_audio: InputAudioContent },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageUrlContent {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InputAudioContent {
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Chat message (OpenAI-compatible) with multi-modal support
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    /// Content can be string (text only) or array of content parts (multi-modal)
    #[serde(default)]
    pub content: Option<ChatContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Chat content - either simple text or array of content parts
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum ChatContent {
    /// Simple text content
    Text(String),
    /// Array of content parts for multi-modal
    Parts(Vec<ChatContentPart>),
}

impl ChatMessage {
    pub fn into_message(self) -> Message {
        let role = match self.role.as_str() {
            "system" => Role::System,
            "assistant" => Role::Assistant,
            _ => Role::User,
        };

        match self.content {
            Some(ChatContent::Text(text)) => Message::new(role, text),
            Some(ChatContent::Parts(parts)) => {
                let content_parts: Vec<crate::models::ContentPart> = parts
                    .into_iter()
                    .filter_map(|p| match p {
                        ChatContentPart::Text { text } => {
                            Some(crate::models::ContentPart::text(text))
                        }
                        ChatContentPart::ImageUrl { image_url } => {
                            Some(crate::models::ContentPart::ImageUrl {
                                url: image_url.url,
                                detail: image_url.detail,
                            })
                        }
                        ChatContentPart::InputAudio { input_audio } => {
                            if let Some(url) = input_audio.url {
                                Some(crate::models::ContentPart::AudioUrl { url })
                            } else if let Some(data) = input_audio.data {
                                Some(crate::models::ContentPart::AudioBase64 {
                                    data,
                                    media_type: input_audio.format,
                                })
                            } else {
                                None
                            }
                        }
                    })
                    .collect();
                Message::with_parts(role, content_parts)
            }
            None => Message::new(role, String::new()),
        }
    }

    #[allow(dead_code)]
    pub fn from_message(msg: &Message) -> Self {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        if msg.content_parts.is_empty() {
            Self {
                role: role.to_string(),
                content: Some(ChatContent::Text(msg.content.clone())),
                name: None,
            }
        } else {
            let parts: Vec<ChatContentPart> = msg
                .content_parts
                .iter()
                .filter_map(|p| match p {
                    crate::models::ContentPart::Text { text } => {
                        Some(ChatContentPart::Text { text: text.clone() })
                    }
                    crate::models::ContentPart::ImageUrl { url, detail } => {
                        Some(ChatContentPart::ImageUrl {
                            image_url: ImageUrlContent {
                                url: url.clone(),
                                detail: detail.clone(),
                            },
                        })
                    }
                    crate::models::ContentPart::ImageBase64 { data, media_type } => {
                        Some(ChatContentPart::ImageUrl {
                            image_url: ImageUrlContent {
                                url: format!(
                                    "data:{};base64,{}",
                                    media_type.as_deref().unwrap_or("image/png"),
                                    data
                                ),
                                detail: None,
                            },
                        })
                    }
                    crate::models::ContentPart::AudioUrl { url } => {
                        Some(ChatContentPart::InputAudio {
                            input_audio: InputAudioContent {
                                url: Some(url.clone()),
                                data: None,
                                format: None,
                            },
                        })
                    }
                    crate::models::ContentPart::AudioBase64 { data, media_type } => {
                        Some(ChatContentPart::InputAudio {
                            input_audio: InputAudioContent {
                                url: None,
                                data: Some(data.clone()),
                                format: media_type.clone(),
                            },
                        })
                    }
                })
                .collect();
            Self {
                role: role.to_string(),
                content: Some(ChatContent::Parts(parts)),
                name: None,
            }
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
    let messages: Vec<Message> = request
        .messages
        .into_iter()
        .map(|m| m.into_message())
        .collect();

    let agent = state.agent.read().await;

    let agent = agent.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Agent not initialized",
                    "type": "service_unavailable",
                    "code": "agent_not_ready"
                }
            })),
        )
    })?;

    let response = agent.chat(messages).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "internal_error",
                    "code": "internal_error"
                }
            })),
        )
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
                content: Some(ChatContent::Text(response.content)),
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
#[allow(dead_code)]
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
            content: Some(ChatContent::Text("Hello".to_string())),
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
        match chat_msg.content {
            Some(ChatContent::Text(text)) => assert_eq!(text, "Test content"),
            _ => panic!("Expected text content"),
        }
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
