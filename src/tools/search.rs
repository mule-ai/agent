//! Search tool - Web search using SearXNG

use crate::tools::{Tool, ToolError, ToolResult};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Search tool configuration
#[derive(Debug, Clone)]
pub struct SearchToolConfig {
    pub searx_url: String,
    pub timeout_seconds: u64,
    pub max_results: usize,
}

impl Default for SearchToolConfig {
    fn default() -> Self {
        Self {
            searx_url: "http://localhost:8088".to_string(),
            timeout_seconds: 30,
            max_results: 10,
        }
    }
}

/// Search tool for web search
#[derive(Clone)]
pub struct SearchTool {
    config: SearchToolConfig,
    client: Client,
}

impl SearchTool {
    pub fn new() -> Self {
        Self::with_config(SearchToolConfig::default())
    }

    pub fn with_config(config: SearchToolConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    /// Perform a search
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let url = format!("{}/search", self.config.searx_url);

        let response = self
            .client
            .get(&url)
            .query(&[("q", query), ("format", "json")])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Search failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct SearxResponse {
            results: Vec<SearxResult>,
        }

        #[derive(Deserialize)]
        struct SearxResult {
            title: String,
            url: String,
            content: Option<String>,
        }

        let searx_response: SearxResponse = response.json().await?;

        let results: Vec<SearchResult> = searx_response
            .results
            .into_iter()
            .take(self.config.max_results)
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.unwrap_or_default(),
            })
            .collect();

        Ok(results)
    }
}

impl Default for SearchTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

impl Tool for SearchTool {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "Search the web for information using SearXNG. Returns a list of search results with titles, URLs, and snippets."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to look up on the web"
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'query' parameter".to_string()))?
            .to_string();

        // Execute search synchronously using tokio runtime
        let results = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.search(&query))
        })
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let content = if results.is_empty() {
            "No results found".to_string()
        } else {
            results
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    format!(
                        "{}. {}\n   URL: {}\n   {}\n",
                        i + 1,
                        r.title,
                        r.url,
                        r.snippet
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ToolResult {
            tool_call_id: String::new(),
            name: self.name().to_string(),
            content,
            success: true,
            error: None,
        })
    }
}
