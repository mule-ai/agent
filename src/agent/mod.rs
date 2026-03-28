//! Agent core module
//! 
//! Implements the agent as specified in SPEC.md

mod session;
mod reasoning;
pub mod llm;
pub mod session_store;
pub mod team;

use itertools::Itertools;

pub use session::{Session, SessionManager};
pub use session_store::SessionStore;
pub use reasoning::ReasoningEngine;
pub use llm::{LlmClient, ToolDefinition};
#[allow(unused)]
pub use team::{AgentTeam, AgentRole, TeamAgent, TeamResponse, TeamAgentResponse, SharedContext};

use crate::config::AppConfig;
use crate::memory::{EmbeddingClient, SqliteMemoryStore};
use crate::models::{Memory, MemoryType, Message, Role, ToolCall};
use crate::tools::{ToolRegistry, ToolResult as ToolExecResult};
use std::sync::Arc;

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub system_prompt: String,
    #[allow(dead_code)]
    pub max_context_length: usize,
    pub enable_reasoning: bool,
    pub reasoning_depth: usize,
    pub enable_memory: bool,
    pub enable_tools: bool,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub reasoning: Option<String>,
    #[allow(dead_code)]
    pub tool_calls: Vec<ToolCall>,
    #[allow(dead_code)]
    pub memory_refs: Vec<String>,
}

/// The main Agent struct
pub struct Agent {
    config: Arc<tokio::sync::RwLock<AppConfig>>,
    agent_config: AgentConfig,
    session_manager: SessionManager,
    reasoning_engine: ReasoningEngine,
    llm_client: Arc<tokio::sync::RwLock<LlmClient>>,
    memory_store: Arc<SqliteMemoryStore>,
    embedding_client: Arc<EmbeddingClient>,
    tool_registry: Arc<ToolRegistry>,
}

impl Agent {
    /// Create a new agent with all dependencies
    pub fn new(
        config: AppConfig,
        agent_config: AgentConfig,
        memory_store: Arc<SqliteMemoryStore>,
        embedding_client: Arc<EmbeddingClient>,
        tool_registry: Arc<ToolRegistry>,
    ) -> anyhow::Result<Self> {
        let session_manager = SessionManager::new();
        let reasoning_engine = ReasoningEngine::with_llm(
            agent_config.reasoning_depth,
            config.model.clone(),
        );
        let llm_client = LlmClient::new(config.model.clone());

        Ok(Self {
            config: Arc::new(tokio::sync::RwLock::new(config)),
            agent_config,
            session_manager,
            reasoning_engine,
            llm_client: Arc::new(tokio::sync::RwLock::new(llm_client)),
            memory_store,
            embedding_client,
            tool_registry,
        })
    }

    /// Update the LLM model configuration (hot-swap)
    /// This allows changing the model at runtime without restarting the agent
    pub async fn update_model(&self, new_model: crate::config::ModelConfig) -> Result<(), AgentError> {
        let current_model = {
            let config = self.config.read().await;
            (config.model.name.clone(), config.model.base_url.clone())
        };
        
        tracing::info!(
            "Hot-swapping model from {}@{} to {}@{}",
            current_model.0,
            current_model.1,
            new_model.name,
            new_model.base_url
        );
        
        // Create new LLM client with updated config
        let new_llm_client = LlmClient::new(new_model.clone());
        
        // Update the config
        {
            let mut config = self.config.write().await;
            config.model = new_model.clone();
        }
        
        // Replace the LLM client (this is safe due to Arc<RwLock>)
        let mut llm_client = self.llm_client.write().await;
        *llm_client = new_llm_client;
        
        tracing::info!("Model hot-swap completed successfully");
        Ok(())
    }

    /// Get current model name
    pub async fn current_model_name(&self) -> String {
        self.config.read().await.model.name.clone()
    }

    /// Get current model configuration
    pub async fn current_model_config(&self) -> crate::config::ModelConfig {
        self.config.read().await.model.clone()
    }

    /// Process a chat message with full memory integration
    pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse, AgentError> {
        // Get or create session
        let session = self.session_manager.get_or_create_session();

        // Add messages to session
        {
            let mut session = session.write();
            for msg in &messages {
                session.add_message(msg.clone());
            }
        }

        // Store conversations in memory if enabled
        if self.agent_config.enable_memory {
            self.store_conversations(&messages).await?;
        }

        // Build context with system prompt if not present
        let context = if messages.first().map(|m| m.role == Role::System).unwrap_or(false) {
            messages.clone()
        } else {
            let mut context = vec![Message::system(self.agent_config.system_prompt.clone())];
            context.extend(messages);
            context
        };

        // Retrieve relevant memories if enabled
        let mut memory_refs = Vec::new();
        let enriched_context = if self.agent_config.enable_memory {
            match self.retrieve_memories(&context).await {
                Ok((enriched, refs)) => {
                    memory_refs = refs;
                    enriched
                }
                Err(e) => {
                    tracing::warn!("Failed to retrieve memories: {}", e);
                    context
                }
            }
        } else {
            context
        };

        // Optional reasoning
        let reasoning = if self.agent_config.enable_reasoning {
            Some(self.reasoning_engine.think(&enriched_context).await)
        } else {
            None
        };

        // Handle tool calls if enabled
        let (response_content, tool_calls) = if self.agent_config.enable_tools {
            self.handle_tool_calls(enriched_context).await?
        } else {
            // Simple LLM call
            let llm_client = self.llm_client.read().await;
            let response = llm_client.chat(enriched_context).await
                .map_err(|e| AgentError::LlmError(e.to_string()))?;
            (response, Vec::new())
        };

        Ok(ChatResponse {
            content: response_content,
            reasoning: reasoning.and_then(|r| r.ok()),
            tool_calls,
            memory_refs,
        })
    }

    /// Store conversations in memory
    async fn store_conversations(&self, messages: &[Message]) -> Result<(), AgentError> {
        for msg in messages {
            let memory = Memory::with_params(
                msg.content.clone(),
                "retrieval".to_string(),
                vec![format!("role:{}", match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                })],
                Some(MemoryType::Conversation),
                false,
            );

            self.memory_store.store(&memory)
                .map_err(|e| AgentError::MemoryError(e.to_string()))?;
        }
        Ok(())
    }

    /// Retrieve relevant memories for context
    async fn retrieve_memories(&self, messages: &[Message]) -> Result<(Vec<Message>, Vec<String>), AgentError> {
        // Get last user message as query
        let query = messages.iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone());

        let query = match query {
            Some(q) => q,
            None => return Ok((messages.to_vec(), Vec::new())),
        };

        // Generate embedding
        let embedding = self.embedding_client.embed(&query)
            .await
            .map_err(|e| AgentError::MemoryError(format!("Embedding failed: {}", e)))?;

        // Query memory store
        let (query_limit, min_similarity) = {
            let config = self.config.read().await;
            (config.memory.query_limit, config.memory.min_similarity)
        };
        let results = self.memory_store.query(
            &embedding,
            "retrieval",
            query_limit,
            min_similarity,
        ).map_err(|e| AgentError::MemoryError(e.to_string()))?;

        // Build context with memories
        let mut enriched = messages.to_vec();
        let mut memory_refs = Vec::new();

        if !results.is_empty() {
            // Add system message with retrieved context
            let memory_context = results.iter()
                .map(|r| {
                    memory_refs.push(r.memory.id.clone());
                    format!("- {} [{}]", r.memory.content, r.memory.id)
                })
                .join("\n");

            enriched.insert(1, Message::system(format!(
                "Relevant memories from your history:\n{}",
                memory_context
            )));
        }

        Ok((enriched, memory_refs))
    }

    /// Handle tool calls from the LLM with proper function calling
    async fn handle_tool_calls(&self, messages: Vec<Message>) -> Result<(String, Vec<ToolCall>), AgentError> {
        // Get tool schemas for function calling
        let tool_schemas = self.get_tool_definitions();
        
        // Call LLM with tools using the proper chat API
        let llm_client = self.llm_client.read().await;
        let result = llm_client.chat_with_tools(messages.clone(), Some(tool_schemas)).await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        // Convert tool calls to our format
        let tool_calls: Vec<ToolCall> = result
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: serde_json::to_string(&tc.function.arguments).unwrap_or_default(),
            })
            .collect();

        // Execute tool calls if present
        let mut tool_results: Vec<ToolExecResult> = Vec::new();
        for tc in &tool_calls {
            if let Some(tool) = self.tool_registry.get(&tc.name) {
                match serde_json::from_str(&tc.arguments) {
                    Ok(args) => {
                        match tool.execute(&args) {
                            Ok(result) => tool_results.push(result),
                            Err(e) => tool_results.push(ToolExecResult::error(
                                tc.id.clone(),
                                tc.name.clone(),
                                e.to_string(),
                            )),
                        }
                    }
                    Err(e) => {
                        tool_results.push(ToolExecResult::error(
                            tc.id.clone(),
                            tc.name.clone(),
                            format!("Invalid arguments: {}", e),
                        ));
                    }
                }
            }
        }

        // If tool calls were made, run another LLM pass with results
        if !tool_results.is_empty() {
            let mut full_context = messages;
            
            // Add tool results as messages in OpenAI format
            for result in &tool_results {
                let content = if result.success {
                    format!("Tool '{}' result:\n{}", result.name, result.content)
                } else {
                    format!("Tool '{}' error:\n{}", result.name, result.error.as_deref().unwrap_or("Unknown error"))
                };
                full_context.push(Message::assistant(content));
            }

            // Final LLM call without tools (to generate the response)
            let llm_client = self.llm_client.read().await;
            let final_response = llm_client.chat(full_context).await
                .map_err(|e| AgentError::LlmError(e.to_string()))?;

            Ok((final_response, tool_calls))
        } else {
            Ok((result.content, Vec::new()))
        }
    }

    /// Get tool definitions for LLM function calling
    fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        let schemas = self.tool_registry.get_function_schemas();
        schemas
            .into_iter()
            .filter_map(|schema| {
                let obj = schema.get("function")?;
                Some(ToolDefinition {
                    name: obj.get("name")?.as_str()?.to_string(),
                    description: obj.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    parameters: obj.get("parameters").cloned().unwrap_or(serde_json::json!({})),
                })
            })
            .collect()
    }

    /// Get current session
    pub fn current_session(&self) -> Option<Session> {
        self.session_manager.current_session()
    }

    /// End current session
    #[allow(dead_code)]
    pub fn end_session(&self) -> Option<Session> {
        self.session_manager.end_current_session()
    }

    /// Get session manager
    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }

    /// Get memory store
    pub fn memory_store(&self) -> Arc<SqliteMemoryStore> {
        self.memory_store.clone()
    }

    /// Get tool registry
    #[allow(dead_code)]
    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }
}

/// Agent errors
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    LlmError(String),
    
    #[error("Session error: {0}")]
    #[allow(dead_code)]
    SessionError(String),
    
    #[error("Memory error: {0}")]
    MemoryError(String),
    
    #[error("Tool error: {0}")]
    #[allow(dead_code)]
    ToolError(String),
    
    #[error("Reasoning error: {0}")]
    #[allow(dead_code)]
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
