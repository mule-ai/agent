//! Tool system for AGI Agent
//! 
//! Implements the tool system as specified in SPEC.md:
//! - Tool trait for all tools
//! - Tool registry for managing tools
//! - Built-in tools: search, bash, read, write, image, fetch

mod bash;
mod fetch;
mod image;
mod read;
mod search;
mod write;

pub use bash::BashTool;
pub use fetch::FetchTool;
pub use image::ImageTool;
pub use read::ReadFileTool;
pub use search::SearchTool;
pub use write::WriteFileTool;


use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Tool error types
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Timeout: {0}")]
    #[allow(dead_code)]
    Timeout(String),
}

/// Tool result
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

/// Tool definition for function calling schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool trait - all tools must implement this
pub trait Tool: Send + Sync {
    /// Tool name
    fn name(&self) -> &str;
    
    /// Tool description
    fn description(&self) -> &str;
    
    /// JSON schema for tool parameters
    fn parameters(&self) -> serde_json::Value;
    
    /// Execute the tool with given arguments
    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError>;
    
    /// Whether the tool is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Tool registry for managing all available tools
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool
    pub fn register<T: Tool + 'static>(&self, tool: T) {
        let name = tool.name().to_string();
        if let Ok(mut tools) = self.tools.write() {
            tools.insert(name.clone(), Arc::new(tool));
            tracing::info!("Registered tool: {}", name);
        }
    }

    /// Unregister a tool (for future use)
    #[allow(dead_code)]
    pub fn unregister(&self, name: &str) -> bool {
        if let Ok(mut tools) = self.tools.write() {
            tools.remove(name).is_some()
        } else {
            false
        }
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        if let Ok(tools) = self.tools.read() {
            tools.get(name).cloned()
        } else {
            None
        }
    }

    /// Check if a tool exists (for future use)
    #[allow(dead_code)]
    pub fn has(&self, name: &str) -> bool {
        if let Ok(tools) = self.tools.read() {
            tools.contains_key(name)
        } else {
            false
        }
    }

    /// List all tool names
    pub fn list_tools(&self) -> Vec<String> {
        if let Ok(tools) = self.tools.read() {
            tools.keys().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get function schemas for all enabled tools
    pub fn get_function_schemas(&self) -> Vec<serde_json::Value> {
        if let Ok(tools) = self.tools.read() {
            tools
                .values()
                .filter(|t| t.is_enabled())
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name(),
                            "description": t.description(),
                            "parameters": t.parameters(),
                        }
                    })
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Execute a tool by name (for future use)
    #[allow(dead_code)]
    pub async fn execute(
        &self,
        name: &str,
        arguments: serde_json::Value,
        tool_call_id: String,
    ) -> Result<ToolResult, ToolError> {
        // Use try_read to avoid blocking in async context
        let tool = {
            let tools = self.tools.read().map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to acquire tool lock: {}", e))
            })?;
            
            let tool = tools
                .get(name)
                .ok_or_else(|| ToolError::NotFound(name.to_string()))?
                .clone();

            if !tool.is_enabled() {
                return Err(ToolError::ExecutionFailed(format!(
                    "Tool '{}' is disabled",
                    name
                )));
            }

            tool
        }; // Lock released here
        
        tool.execute(&arguments).map(|result| {
            // Update tool_call_id if not set
            ToolResult {
                tool_call_id,
                ..result
            }
        })
    }

    /// Create default registry with all built-in tools
    pub fn default_registry() -> Self {
        let registry = Self::new();
        
        // Register built-in tools
        registry.register(crate::tools::SearchTool::new());
        registry.register(crate::tools::BashTool::new());
        registry.register(crate::tools::ReadFileTool::new());
        registry.register(crate::tools::WriteFileTool::new());
        registry.register(crate::tools::ImageTool::new());
        registry.register(crate::tools::FetchTool::new());
        
        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
