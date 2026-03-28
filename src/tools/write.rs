//! Write file tool

use crate::tools::{Tool, ToolError, ToolResult};
use std::path::Path;
use tokio::fs;

/// Write file tool configuration
#[derive(Debug, Clone)]
pub struct WriteFileToolConfig {
    /// Allowed base directories for writing
    pub allowed_dirs: Vec<String>,
    /// Whether to allow creating new files (for future use)
    #[allow(dead_code)]
    pub allow_create: bool,
    /// Whether to allow overwriting existing files
    pub allow_overwrite: bool,
    /// Maximum file size to write in bytes
    pub max_file_size: usize,
}

impl Default for WriteFileToolConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec![],
            allow_create: true,
            allow_overwrite: false,
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Tool for writing files
#[derive(Clone)]
pub struct WriteFileTool {
    config: WriteFileToolConfig,
}

impl WriteFileTool {
    pub fn new() -> Self {
        Self::with_config(WriteFileToolConfig::default())
    }

    pub fn with_config(config: WriteFileToolConfig) -> Self {
        Self { config }
    }

    /// Write file contents
    pub async fn write_file(&self, path: &str, content: &str) -> Result<(), ToolError> {
        let path = Path::new(path);

        // Security check: verify path is allowed
        if !self.is_path_allowed(path) {
            return Err(ToolError::PermissionDenied(format!(
                "Path not allowed: {:?}",
                path
            )));
        }

        // Check content size
        if content.len() > self.config.max_file_size {
            return Err(ToolError::ExecutionFailed(format!(
                "Content too large: {} bytes (max: {})",
                content.len(),
                self.config.max_file_size
            )));
        }

        // Check if file exists
        if path.exists() {
            if !self.config.allow_overwrite {
                return Err(ToolError::ExecutionFailed(format!(
                    "File already exists and overwriting is disabled: {:?}",
                    path
                )));
            }
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
            })?;
        }

        // Write file
        fs::write(path, content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        Ok(())
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

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, or overwrites if it does (if allowed)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' parameter".to_string()))?
            .to_string();

        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'content' parameter".to_string()))?
            .to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.write_file(&path, &content))
        })?;

        Ok(ToolResult {
            tool_call_id: String::new(),
            name: self.name().to_string(),
            content: format!("Successfully wrote {} bytes to {}", content.len(), path),
            success: true,
            error: None,
        })
    }
}
