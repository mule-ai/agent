//! LLM client for communicating with llama.cpp OpenAI-compatible API
//!
//! Calls llama-server for chat completions with optional tool support

use crate::config::ModelConfig;
use crate::models::Message;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool call returned by the LLM
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "function")]
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// LLM client for llama.cpp API
pub struct LlmClient {
    config: ModelConfig,
    client: Client,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Send a chat request to the LLM
    pub async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let result = self.chat_with_tools(messages, None).await?;
        Ok(result.content)
    }

    /// Send a chat request with tools to the LLM
    pub async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResult> {
        let api_messages: Vec<serde_json::Value> = messages.iter().map(|m| m.to_openai()).collect();

        let request = ChatRequest {
            model: self.config.name.clone(),
            messages: api_messages,
            stream: false,
            tools: tools.map(|t| {
                t.into_iter()
                    .map(|tool| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": tool.name,
                                "description": tool.description,
                                "parameters": tool.parameters,
                            }
                        })
                    })
                    .collect()
            }),
            tool_choice: Some(serde_json::json!("auto")),
        };

        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to LLM")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error: {} - {}", status, error);
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("Failed to parse LLM response")?;

        let choice = chat_response.choices.into_iter().next().unwrap_or_default();

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|tc| ToolCall {
                    id: tc.id,
                    function: ToolCallFunction {
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    },
                })
                .collect()
        });

        Ok(ChatResult {
            content: choice.message.content,
            tool_calls,
        })
    }

}

/// Result from a chat completion
#[derive(Debug, Clone)]
pub struct ChatResult {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

impl Default for Choice {
    fn default() -> Self {
        Choice {
            message: ResponseMessage {
                content: String::new(),
                tool_calls: None,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    id: String,
    #[serde(rename = "function")]
    function: ResponseToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct ResponseToolCallFunction {
    name: String,
    arguments: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_client_config() {
        let config = ModelConfig {
            base_url: "http://localhost:8081".to_string(),
            name: "qwen3.5-4b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            embedding_dim: 768,
            max_tokens: 4096,
            api_key: None,
        };

        let client = LlmClient::new(config);
        assert_eq!(client.config.name, "qwen3.5-4b");
        assert!(client.config.base_url.contains("8081"));
    }
}
