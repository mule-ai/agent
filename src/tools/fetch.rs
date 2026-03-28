//! Fetch tool - Retrieve web content from URLs

use crate::knowledge::WebFetcher;
use crate::tools::{Tool, ToolError, ToolResult};
use anyhow::Result;

/// Fetch tool for retrieving web content
#[derive(Clone)]
pub struct FetchTool {
    fetcher: WebFetcher,
    extract_article: bool,
}

impl FetchTool {
    pub fn new() -> Self {
        Self {
            fetcher: WebFetcher::new(),
            extract_article: false,
        }
    }

    pub fn with_article_extraction(enabled: bool) -> Self {
        Self {
            fetcher: WebFetcher::new(),
            extract_article: enabled,
        }
    }

    /// Fetch a URL and return the content
    pub fn fetch_url(&self, url: &str) -> Result<ToolResult, ToolError> {
        let fetcher = self.fetcher.clone();
        let extract_article = self.extract_article;
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                if extract_article {
                    fetcher.fetch_article(url).await
                } else {
                    fetcher.fetch(url).await
                }
            })
        }).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult {
            tool_call_id: String::new(),
            name: "fetch".to_string(),
            content: format!(
                "Title: {}\n\nSource: {}\n\nContent:\n{}",
                result.title,
                result.url.unwrap_or_default(),
                result.content
            ),
            success: true,
            error: None,
        })
    }
}

impl Default for FetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FetchTool {
    fn name(&self) -> &str {
        "fetch"
    }

    fn description(&self) -> &str {
        "Fetch the content of a webpage from a URL. Extracts the main text content from the page, stripping HTML tags and scripts. Returns the title and content."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                },
                "extract_article": {
                    "type": "boolean",
                    "description": "Extract only article paragraphs (longer text blocks). Default: false",
                    "default": false
                }
            },
            "required": ["url"]
        })
    }

    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let url = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'url' parameter".to_string()))?
            .to_string();

        // Check for extract_article flag
        let extract_article = arguments
            .get("extract_article")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let tool = if extract_article {
            Self::with_article_extraction(true)
        } else {
            Self::new()
        };

        tool.fetch_url(&url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_tool_name() {
        let tool = FetchTool::new();
        assert_eq!(tool.name(), "fetch");
    }

    #[test]
    fn test_fetch_tool_description() {
        let tool = FetchTool::new();
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("URL"));
    }

    #[test]
    fn test_fetch_tool_parameters() {
        let tool = FetchTool::new();
        let params = tool.parameters();
        
        assert!(params.is_object());
        assert!(params.get("properties").is_some());
        assert!(params.pointer("/properties/url").is_some());
    }

    #[tokio::test]
    async fn test_fetch_tool_disabled() {
        let tool = FetchTool::new();
        // Note: Without a real URL, this would fail, but we're just testing the method exists
        assert!(tool.is_enabled());
    }
}
