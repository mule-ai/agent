//! Read file tool

use crate::tools::{Tool, ToolError, ToolResult};
use std::path::Path;
use tokio::fs;

/// Read file tool configuration
#[derive(Debug, Clone)]
pub struct ReadFileToolConfig {
    /// Allowed base directories for reading
    pub allowed_dirs: Vec<String>,
    /// Maximum file size to read in bytes
    pub max_file_size: usize,
}

impl Default for ReadFileToolConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec![],
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// Tool for reading files
#[derive(Clone)]
pub struct ReadFileTool {
    config: ReadFileToolConfig,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self::with_config(ReadFileToolConfig::default())
    }

    pub fn with_config(config: ReadFileToolConfig) -> Self {
        Self { config }
    }

    /// Read file contents
    pub async fn read_file(&self, path: &str) -> Result<String, ToolError> {
        let path = Path::new(path);

        // Security check: verify path is allowed
        if !self.is_path_allowed(path) {
            return Err(ToolError::PermissionDenied(format!(
                "Path not allowed: {:?}",
                path
            )));
        }

        // Check file size
        let metadata = fs::metadata(path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read metadata: {}", e)))?;

        if metadata.len() as usize > self.config.max_file_size {
            return Err(ToolError::ExecutionFailed(format!(
                "File too large: {} bytes (max: {})",
                metadata.len(),
                self.config.max_file_size
            )));
        }

        // Read file
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        Ok(content)
    }

    /// Check if a path is allowed
    fn is_path_allowed(&self, path: &Path) -> bool {
        if self.config.allowed_dirs.is_empty() {
            return true; // All paths allowed if no restrictions
        }

        let path_str = path.to_string_lossy();
        for allowed in &self.config.allowed_dirs {
            if path_str.starts_with(allowed) || allowed == "/" {
                return true;
            }
        }

        false
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Returns the file contents as text."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' parameter".to_string()))?
            .to_string();

        let content = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.read_file(&path))
        })?;

        Ok(ToolResult {
            tool_call_id: String::new(),
            name: self.name().to_string(),
            content,
            success: true,
            error: None,
        })
    }
}
