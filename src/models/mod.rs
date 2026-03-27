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

    pub fn with_user(user_id: String) -> Self {
        Self {
            user_id: Some(user_id),
            ..Self::new()
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn add_memory(&mut self, memory_id: String) {
        self.memories.push(memory_id);
    }

    pub fn end(&mut self) {
        self.status = SessionStatus::Ended;
        self.ended_at = Some(Utc::now());
    }

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
    pub fn success(tool_call_id: String, name: String, content: String) -> Self {
        Self {
            tool_call_id,
            name,
            content,
            success: true,
            error: None,
        }
    }

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

/// Message model as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: String,
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

    pub fn with_tool_calls(mut self, calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(calls);
        self
    }

    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }
}

/// Training source as defined in SPEC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrainingSource {
    Session,
    Search,
    Manual,
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
        let msg = Message::user("Search for something".to_string())
            .with_tool_calls(vec![tool_call]);
        
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.unwrap().len(), 1);
    }

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success("call-1".to_string(), "search".to_string(), "Found results".to_string());
        assert!(success.success);
        assert_eq!(success.content, "Found results");
        assert!(success.error.is_none());
        
        let failure = ToolResult::error("call-2".to_string(), "bash".to_string(), "Command failed".to_string());
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
