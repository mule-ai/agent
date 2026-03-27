//! Agent core module
//! 
//! Implements the agent as specified in SPEC.md

mod session;
mod reasoning;
mod llm;

pub use session::{Session, SessionManager};
pub use reasoning::ReasoningEngine;
pub use llm::LlmClient;

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub system_prompt: String,
    pub max_context_length: usize,
    pub enable_reasoning: bool,
    pub reasoning_depth: usize,
    pub enable_memory: bool,
    pub enable_tools: bool,
    pub max_tool_calls: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            system_prompt: Self::default_system_prompt(),
            max_context_length: 8192,
            enable_reasoning: true,
            reasoning_depth: 3,
            enable_memory: true,
            enable_tools: true,
            max_tool_calls: 10,
        }
    }
}

impl AgentConfig {
    pub fn default_system_prompt() -> String {
        r#"You are an AI assistant with extensive memory and learning capabilities.

You have access to:
- Long-term memory that persists across conversations
- Ability to search for information on the web
- File system access for reading and writing

Be helpful, concise, and accurate. Use your tools when appropriate."#.to_string()
    }
}

/// Chat response from agent
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<crate::models::ToolCall>,
    pub memory_refs: Vec<String>,
}

/// The main Agent struct
pub struct Agent {
    _config: crate::config::AppConfig,
    agent_config: AgentConfig,
    session_manager: SessionManager,
    reasoning_engine: ReasoningEngine,
    llm_client: LlmClient,
}

impl Agent {
    /// Create a new agent
    pub fn new(config: crate::config::AppConfig, agent_config: AgentConfig) -> anyhow::Result<Self> {
        let session_manager = SessionManager::new();
        let reasoning_engine = ReasoningEngine::new(agent_config.reasoning_depth);
        let llm_client = LlmClient::new(config.model.clone());

        Ok(Self {
            _config: config,
            agent_config,
            session_manager,
            reasoning_engine,
            llm_client,
        })
    }

    /// Process a chat message
    pub async fn chat(&self, messages: Vec<crate::models::Message>) -> Result<ChatResponse, AgentError> {
        // Get or create session
        let session = self.session_manager.get_or_create_session();

        // Add messages to session
        {
            let mut session = session.write();
            for msg in &messages {
                session.add_message(msg.clone());
            }
        }

        // Build context with system prompt
        let mut context = vec![crate::models::Message::system(self.agent_config.system_prompt.clone())];
        context.extend(messages);

        // Optional reasoning
        let reasoning = if self.agent_config.enable_reasoning {
            Some(self.reasoning_engine.think(&context).await)
        } else {
            None
        };

        // Call LLM
        let response = self.llm_client.chat(context).await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        Ok(ChatResponse {
            content: response,
            reasoning: reasoning.and_then(|r| r.ok()),
            tool_calls: Vec::new(),
            memory_refs: Vec::new(),
        })
    }

    /// Get current session
    pub fn current_session(&self) -> Option<Session> {
        self.session_manager.current_session()
    }
}

/// Agent errors
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    LlmError(String),
    
    #[error("Session error: {0}")]
    SessionError(String),
    
    #[error("Memory error: {0}")]
    MemoryError(String),
    
    #[error("Tool error: {0}")]
    ToolError(String),
    
    #[error("Reasoning error: {0}")]
    ReasoningError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        
        assert!(!config.system_prompt.is_empty());
        assert_eq!(config.max_context_length, 8192);
        assert!(config.enable_reasoning);
        assert_eq!(config.reasoning_depth, 3);
        assert!(config.enable_memory);
        assert!(config.enable_tools);
        assert_eq!(config.max_tool_calls, 10);
    }

    #[test]
    fn test_agent_config_custom() {
        let config = AgentConfig {
            system_prompt: "Custom prompt".to_string(),
            max_context_length: 4096,
            enable_reasoning: false,
            reasoning_depth: 5,
            enable_memory: false,
            enable_tools: false,
            max_tool_calls: 5,
        };

        assert_eq!(config.system_prompt, "Custom prompt");
        assert_eq!(config.max_context_length, 4096);
        assert!(!config.enable_reasoning);
        assert_eq!(config.reasoning_depth, 5);
        assert!(!config.enable_memory);
    }

    #[test]
    fn test_chat_response_default() {
        let response = ChatResponse {
            content: "Hello!".to_string(),
            reasoning: None,
            tool_calls: Vec::new(),
            memory_refs: Vec::new(),
        };

        assert_eq!(response.content, "Hello!");
        assert!(response.reasoning.is_none());
        assert!(response.tool_calls.is_empty());
        assert!(response.memory_refs.is_empty());
    }

    #[test]
    fn test_chat_response_with_reasoning() {
        let response = ChatResponse {
            content: "The answer is 42.".to_string(),
            reasoning: Some("I calculated this step by step...".to_string()),
            tool_calls: Vec::new(),
            memory_refs: vec!["mem-1".to_string()],
        };

        assert!(response.reasoning.is_some());
        assert!(!response.memory_refs.is_empty());
    }

    #[test]
    fn test_agent_error_display() {
        let err = AgentError::LlmError("Connection failed".to_string());
        assert_eq!(err.to_string(), "LLM error: Connection failed");

        let err = AgentError::MemoryError("Memory not found".to_string());
        assert_eq!(err.to_string(), "Memory error: Memory not found");
    }
}
