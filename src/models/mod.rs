//! Data models for AGI Agent
//!
//! These models match the SPEC.md definitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Memory types as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Fact,
    Concept,
    Conversation,
    ToolResult,
}

impl Default for MemoryType {
    fn default() -> Self {
        MemoryType::Fact
    }
}

/// Memory model as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub namespace: String,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub memory_type: MemoryType,
    #[serde(default)]
    pub evict_to_training: bool,
    #[serde(default)]
    pub is_persistent: bool,
}

impl Memory {
    /// Create a new memory with generated ID and timestamps
    pub fn new(content: String, namespace: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            content,
            embedding: Vec::new(),
            namespace,
            tags: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            memory_type: MemoryType::default(),
            evict_to_training: false,
            is_persistent: false,
        }
    }

    /// Create a memory with all parameters
    pub fn with_params(
        content: String,
        namespace: String,
        tags: Vec<String>,
        memory_type: Option<MemoryType>,
        evict_to_training: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            content,
            embedding: Vec::new(),
            namespace,
            tags,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            memory_type: memory_type.unwrap_or_default(),
            evict_to_training,
            is_persistent: false,
        }
    }
}

/// Session status as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Ended,
    Error,
}

impl Default for SessionStatus {
    fn default() -> Self {
        SessionStatus::Active
    }
}

/// Session summary for listing (without full message history)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub status: SessionStatus,
    pub message_count: usize,
}

impl SessionSummary {
    /// Create a summary from a full session
    pub fn from_session(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            user_id: session.user_id.clone(),
            created_at: session.created_at,
            ended_at: session.ended_at,
            status: session.status.clone(),
            message_count: session.messages.len(),
        }
    }
}

/// Session model as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: Option<String>,
    pub messages: Vec<Message>,
    pub memories: Vec<String>, // Memory IDs used in session
    pub created_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub status: SessionStatus,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: None,
            messages: Vec::new(),
            memories: Vec::new(),
            created_at: Utc::now(),
            ended_at: None,
            status: SessionStatus::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_user(user_id: String) -> Self {
        Self {
            user_id: Some(user_id),
            ..Self::new()
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    #[allow(dead_code)]
    pub fn add_memory(&mut self, memory_id: String) {
        self.memories.push(memory_id);
    }

    pub fn end(&mut self) {
        self.status = SessionStatus::Ended;
        self.ended_at = Some(Utc::now());
    }

    #[allow(dead_code)]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Message role as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Default for Role {
    fn default() -> Self {
        Role::User
    }
}

/// Tool call as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String, // JSON string of arguments
}

impl ToolCall {
    #[allow(dead_code)]
    pub fn new(name: String, arguments: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            arguments,
        }
    }
}

/// Tool result as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
    pub success: bool,
    pub error: Option<String>,
}

impl ToolResult {
    #[allow(dead_code)]
    pub fn success(tool_call_id: String, name: String, content: String) -> Self {
        Self {
            tool_call_id,
            name,
            content,
            success: true,
            error: None,
        }
    }

    #[allow(dead_code)]
    pub fn error(tool_call_id: String, name: String, error: String) -> Self {
        Self {
            tool_call_id,
            name,
            content: String::new(),
            success: false,
            error: Some(error),
        }
    }
}

/// Content part for multi-modal messages
/// Supports text, images (URL or base64), and audio (URL or base64)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    /// Plain text content
    Text { text: String },
    /// Image from URL
    ImageUrl {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// Image as base64-encoded data
    ImageBase64 {
        data: String,
        media_type: Option<String>,
    },
    /// Audio from URL
    AudioUrl { url: String },
    /// Audio as base64-encoded data
    AudioBase64 {
        data: String,
        media_type: Option<String>,
    },
}

impl ContentPart {
    /// Create a text content part
    pub fn text(text: impl Into<String>) -> Self {
        ContentPart::Text { text: text.into() }
    }

    /// Create an image URL content part (for future use)
    #[allow(dead_code)]
    pub fn image_url(url: impl Into<String>) -> Self {
        ContentPart::ImageUrl {
            url: url.into(),
            detail: None,
        }
    }

    /// Create an image URL content part with detail level (for future use)
    #[allow(dead_code)]
    pub fn image_url_with_detail(url: impl Into<String>, detail: impl Into<String>) -> Self {
        ContentPart::ImageUrl {
            url: url.into(),
            detail: Some(detail.into()),
        }
    }

    /// Create an image base64 content part (for future use)
    #[allow(dead_code)]
    pub fn image_base64(data: impl Into<String>, media_type: Option<String>) -> Self {
        ContentPart::ImageBase64 {
            data: data.into(),
            media_type,
        }
    }

    /// Convert to OpenAI content format
    pub fn to_openai(&self) -> serde_json::Value {
        match self {
            ContentPart::Text { text } => serde_json::json!({
                "type": "text",
                "text": text
            }),
            ContentPart::ImageUrl { url, detail } => {
                let mut obj = serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": url
                    }
                });
                if let Some(d) = detail {
                    obj["image_url"]["detail"] = serde_json::json!(d);
                }
                obj
            }
            ContentPart::ImageBase64 { data, media_type } => {
                serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}",
                            media_type.as_deref().unwrap_or("image/png"),
                            data)
                    }
                })
            }
            ContentPart::AudioUrl { url } => serde_json::json!({
                "type": "input_audio",
                "input_audio": {
                    "url": url
                }
            }),
            ContentPart::AudioBase64 { data, media_type } => serde_json::json!({
                "type": "input_audio",
                "input_audio": {
                    "data": format!("data:{};base64,{}",
                        media_type.as_deref().unwrap_or("audio/wav"),
                        data),
                    "format": media_type.as_deref().unwrap_or("wav")
                }
            }),
        }
    }
}

/// Message model as defined in SPEC
/// Now supports multi-modal content with Vec<ContentPart>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    /// Content can be a single string (backward compatible) or Vec<ContentPart>
    #[serde(default)]
    pub content: String,
    /// Multi-modal content parts (takes precedence over content if non-empty)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub content_parts: Vec<ContentPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<ToolResult>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub memory_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

impl Message {
    pub fn new(role: Role, content: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content,
            content_parts: Vec::new(),
            tool_calls: None,
            tool_results: None,
            memory_refs: Vec::new(),
            reasoning: None,
        }
    }

    /// Create a multi-modal message with content parts
    pub fn with_parts(role: Role, parts: Vec<ContentPart>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content: parts
                .iter()
                .filter_map(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            content_parts: parts,
            tool_calls: None,
            tool_results: None,
            memory_refs: Vec::new(),
            reasoning: None,
        }
    }

    pub fn system(content: String) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: String) -> Self {
        Self::new(Role::User, content)
    }

    pub fn assistant(content: String) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Create a user message with image (for future use)
    #[allow(dead_code)]
    pub fn user_with_image(content: String, image_url: String) -> Self {
        let parts = vec![
            ContentPart::text(content),
            ContentPart::image_url(image_url),
        ];
        Self::with_parts(Role::User, parts)
    }

    #[allow(dead_code)]
    pub fn with_tool_calls(mut self, calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(calls);
        self
    }

    #[allow(dead_code)]
    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }

    /// Get the text content (backward compatible)
    #[allow(dead_code)]
    pub fn get_text(&self) -> String {
        if !self.content_parts.is_empty() {
            self.content_parts
                .iter()
                .filter_map(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            self.content.clone()
        }
    }

    /// Check if message has multi-modal content
    #[allow(dead_code)]
    pub fn has_multimodal_content(&self) -> bool {
        !self.content_parts.is_empty()
            || self.content.contains("data:image")
            || self.content.contains("data:audio")
    }

    /// Convert to OpenAI message format for API calls
    pub fn to_openai(&self) -> serde_json::Value {
        if self.content_parts.is_empty() {
            // Simple text message
            serde_json::json!({
                "role": match self.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                "content": self.content
            })
        } else {
            // Multi-modal message
            let content: Vec<serde_json::Value> =
                self.content_parts.iter().map(|p| p.to_openai()).collect();
            serde_json::json!({
                "role": match self.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                "content": content
            })
        }
    }
}

/// Training source as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrainingSource {
    Session,
    Search,
    Manual,
    Memory,
}

impl Default for TrainingSource {
    fn default() -> Self {
        TrainingSource::Session
    }
}

/// Training example as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    pub id: String,
    pub prompt: String,
    pub completion: String,
    pub reasoning: String,
    pub reward: f32,
    #[serde(default)]
    pub source: TrainingSource,
    pub created_at: DateTime<Utc>,
    pub quality_score: f32,
    #[serde(default)]
    pub used_in_training: bool,
}

impl TrainingExample {
    pub fn new(prompt: String, completion: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            prompt,
            completion,
            reasoning: String::new(),
            reward: 0.0,
            source: TrainingSource::default(),
            created_at: Utc::now(),
            quality_score: 0.0,
            used_in_training: false,
        }
    }
}

/// Training job status as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrainingStatus {
    Pending,
    Training,
    Completed,
    Failed,
}

impl Default for TrainingStatus {
    fn default() -> Self {
        TrainingStatus::Pending
    }
}

impl std::fmt::Display for TrainingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrainingStatus::Pending => write!(f, "pending"),
            TrainingStatus::Training => write!(f, "training"),
            TrainingStatus::Completed => write!(f, "completed"),
            TrainingStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Training job as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingJob {
    pub id: String,
    pub status: TrainingStatus,
    pub epochs: usize,
    pub current_epoch: usize,
    pub current_step: usize,
    pub total_steps: usize,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl TrainingJob {
    pub fn new(epochs: usize, total_steps: usize) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            status: TrainingStatus::Pending,
            epochs,
            current_epoch: 0,
            current_step: 0,
            total_steps,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
        }
    }

    pub fn start(&mut self) {
        self.status = TrainingStatus::Training;
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self) {
        self.status = TrainingStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    pub fn fail(&mut self, error: String) {
        self.status = TrainingStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error);
    }
}

/// Query result with score as returned by memory retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub memory: Memory,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let memory = Memory::new("Test content".to_string(), "retrieval".to_string());
        assert_eq!(memory.namespace, "retrieval");
        assert_eq!(memory.content, "Test content");
        assert!(!memory.id.is_empty());
    }

    #[test]
    fn test_memory_with_params() {
        let memory = Memory::with_params(
            "Concept content".to_string(),
            "training".to_string(),
            vec!["concept".to_string()],
            Some(MemoryType::Concept),
            true,
        );
        assert_eq!(memory.namespace, "training");
        assert_eq!(memory.memory_type, MemoryType::Concept);
        assert!(memory.evict_to_training);
    }

    #[test]
    fn test_session_lifecycle() {
        let mut session = Session::new();
        assert_eq!(session.status, SessionStatus::Active);

        session.add_message(Message::user("Hello".to_string()));
        assert_eq!(session.messages.len(), 1);

        session.end();
        assert_eq!(session.status, SessionStatus::Ended);
        assert!(session.ended_at.is_some());
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::new(Role::User, "Hello".to_string());
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");

        let assistant = Message::assistant("Hi there".to_string());
        assert_eq!(assistant.role, Role::Assistant);

        let system = Message::system("You are helpful".to_string());
        assert_eq!(system.role, Role::System);
    }

    #[test]
    fn test_message_with_tool_calls() {
        let tool_call = ToolCall::new("search".to_string(), r#"{"query":"test"}"#.to_string());
        let msg =
            Message::user("Search for something".to_string()).with_tool_calls(vec![tool_call]);

        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.unwrap().len(), 1);
    }

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success(
            "call-1".to_string(),
            "search".to_string(),
            "Found results".to_string(),
        );
        assert!(success.success);
        assert_eq!(success.content, "Found results");
        assert!(success.error.is_none());

        let failure = ToolResult::error(
            "call-2".to_string(),
            "bash".to_string(),
            "Command failed".to_string(),
        );
        assert!(!failure.success);
        assert_eq!(failure.error, Some("Command failed".to_string()));
    }

    #[test]
    fn test_training_job_lifecycle() {
        let mut job = TrainingJob::new(3, 125);
        assert_eq!(job.status, TrainingStatus::Pending);

        job.start();
        assert_eq!(job.status, TrainingStatus::Training);
        assert!(job.started_at.is_some());

        job.complete();
        assert_eq!(job.status, TrainingStatus::Completed);
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn test_training_example_creation() {
        let example = TrainingExample::new("Prompt".to_string(), "Completion".to_string());
        assert_eq!(example.prompt, "Prompt");
        assert_eq!(example.completion, "Completion");
        assert!(!example.used_in_training);
    }

    #[test]
    fn test_role_serialization() {
        let user = Role::User;
        let json = serde_json::to_string(&user).unwrap();
        assert_eq!(json, "\"user\"");

        let deserialized: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Role::User);
    }

    #[test]
    fn test_memory_type_serialization() {
        let concept = MemoryType::Concept;
        let json = serde_json::to_string(&concept).unwrap();
        assert_eq!(json, "\"concept\"");
    }

    #[test]
    fn test_session_with_user() {
        let session = Session::with_user("user-123".to_string());
        assert_eq!(session.user_id, Some("user-123".to_string()));
    }
}
