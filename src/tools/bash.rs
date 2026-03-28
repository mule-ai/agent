//! Bash tool - Execute shell commands

use crate::tools::{Tool, ToolError, ToolResult};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

/// Bash tool configuration
#[derive(Debug, Clone)]
pub struct BashToolConfig {
    /// Allowed directory paths
    pub allowed_dirs: Vec<String>,
    /// Maximum execution time in seconds (for future use)
    #[allow(dead_code)]
    pub timeout_seconds: u64,
    /// Whether to allow dangerous commands
    pub allow_dangerous: bool,
}

impl Default for BashToolConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec![],
            timeout_seconds: 60,
            allow_dangerous: false,
        }
    }
}

/// Bash tool for executing shell commands
#[derive(Clone)]
pub struct BashTool {
    config: BashToolConfig,
}

impl BashTool {
    pub fn new() -> Self {
        Self::with_config(BashToolConfig::default())
    }

    pub fn with_config(config: BashToolConfig) -> Self {
        Self { config }
    }

    /// Execute a bash command
    pub async fn execute_command(
        &self,
        command: &str,
        working_dir: Option<&str>,
    ) -> Result<CommandOutput, ToolError> {
        // Security check: block dangerous commands unless explicitly allowed
        if !self.config.allow_dangerous {
            let dangerous = [
                "rm -rf /",
                "rm -rf /*",
                ":(){:|:&};:", // Fork bomb
                "mkfs",
                "dd if=",
            ];

            for pattern in dangerous {
                if command.contains(pattern) {
                    return Err(ToolError::PermissionDenied(format!(
                        "Dangerous command pattern blocked: {}",
                        pattern
                    )));
                }
            }
        }

        // Parse command
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(ToolError::InvalidArguments("Empty command".to_string()));
        }

        let program = parts[0];
        let args = &parts[1..];

        // Build command
        let mut cmd = Command::new(program);

        // Set working directory
        if let Some(dir) = working_dir {
            // Check if working dir is allowed
            if !self.is_dir_allowed(dir) {
                return Err(ToolError::PermissionDenied(format!(
                    "Directory not allowed: {}",
                    dir
                )));
            }
            cmd.current_dir(dir);
        } else {
            // Use current directory
            if let Ok(cwd) = std::env::current_dir() {
                if !self.is_dir_allowed(cwd.to_str().unwrap_or("")) {
                    return Err(ToolError::PermissionDenied(format!(
                        "Current directory not allowed: {:?}",
                        cwd
                    )));
                }
            }
        }

        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        // Set timeout
        cmd.kill_on_drop(true);

        let output = cmd
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to execute: {}", e)))?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
        })
    }

    /// Check if a directory is allowed
    fn is_dir_allowed(&self, dir: &str) -> bool {
        if self.config.allowed_dirs.is_empty() {
            return true; // All directories allowed if no restrictions
        }

        for allowed in &self.config.allowed_dirs {
            if dir.starts_with(allowed) || allowed == "/" {
                return true;
            }
        }

        false
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Command output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Returns stdout, stderr, and exit code. Use with caution - dangerous commands are blocked."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory for the command"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let command = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'command' parameter".to_string()))?
            .to_string();

        let working_dir = arguments.get("working_dir").and_then(|v| v.as_str());

        // Execute synchronously
        let output = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.execute_command(&command, working_dir))
        })?;

        let content = format!(
            "Exit code: {}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
            output.exit_code, output.stdout, output.stderr
        );

        Ok(ToolResult {
            tool_call_id: String::new(),
            name: self.name().to_string(),
            content,
            success: output.success,
            error: if output.success {
                None
            } else {
                Some(format!(
                    "Command failed with exit code {}",
                    output.exit_code
                ))
            },
        })
    }
}
