//! Image tool for fetching and analyzing images
//!
//! This tool allows the agent to fetch images from URLs and describe their contents

use crate::tools::{Tool, ToolError, ToolResult};
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

/// Tool for working with images
pub struct ImageTool;

impl ImageTool {
    pub fn new() -> Self {
        Self
    }

    /// Fetch image from URL and return as base64
    async fn fetch_image_as_base64(url: &str) -> Result<(String, String)> {
        let response = reqwest::get(url).await?;
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .to_string();

        let bytes = response.bytes().await?;
        let base64 = base64_encode(&bytes);

        Ok((base64, content_type))
    }

    /// Read image from local file and return as base64
    fn read_image_file(path: &Path) -> Result<(String, String)> {
        let bytes = std::fs::read(path)?;
        let base64 = base64_encode(&bytes);

        // Infer media type from extension
        let media_type = match path.extension().and_then(|e| e.to_str()) {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            Some("svg") => "image/svg+xml",
            Some("bmp") => "image/bmp",
            _ => "application/octet-stream",
        }
        .to_string();

        Ok((base64, media_type))
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as i32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as i32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as i32;

        result.push(ALPHABET[((b0 >> 2) & 0x3F) as usize] as char);
        result.push(ALPHABET[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[(((b1 << 2) | (b2 >> 6)) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(b2 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[derive(Debug, Serialize)]
struct ImageInfo {
    url: String,
    data: Option<String>,
    media_type: String,
    size_bytes: usize,
}

impl Tool for ImageTool {
    fn name(&self) -> &str {
        "fetch_image"
    }

    fn description(&self) -> &str {
        "Fetch an image from a URL or read a local image file and return its metadata and optionally the base64-encoded data for vision model analysis."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "enum": ["url", "file"],
                    "description": "Source of the image"
                },
                "url": {
                    "type": "string",
                    "description": "URL of the image to fetch (required if source is 'url')"
                },
                "path": {
                    "type": "string",
                    "description": "Path to local image file (required if source is 'file')"
                },
                "include_data": {
                    "type": "boolean",
                    "description": "Whether to include the base64-encoded image data",
                    "default": false
                }
            },
            "required": ["source"]
        })
    }

    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let source = arguments["source"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'source' field".to_string()))?;

        let include_data = arguments["include_data"].as_bool().unwrap_or(false);

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create runtime: {}", e)))?;

        let result = match source {
            "url" => {
                let url = arguments["url"].as_str().ok_or_else(|| {
                    ToolError::InvalidArguments("Missing 'url' field for URL source".to_string())
                })?;

                runtime
                    .block_on(Self::fetch_image_as_base64(url))
                    .map(|(data, media_type)| {
                        let info = ImageInfo {
                            url: url.to_string(),
                            data: if include_data {
                                Some(data.clone())
                            } else {
                                None
                            },
                            media_type,
                            size_bytes: data.len() * 3 / 4, // Approximate
                        };
                        serde_json::to_string_pretty(&info).unwrap_or_default()
                    })
            }
            "file" => {
                let path_str = arguments["path"].as_str().ok_or_else(|| {
                    ToolError::InvalidArguments("Missing 'path' field for file source".to_string())
                })?;

                let path = Path::new(path_str);
                if !path.exists() {
                    return Err(ToolError::ExecutionFailed(format!(
                        "File not found: {}",
                        path.display()
                    )));
                }

                Self::read_image_file(path).map(|(data, media_type)| {
                    let info = ImageInfo {
                        url: format!("file://{}", path.display()),
                        data: if include_data {
                            Some(data.clone())
                        } else {
                            None
                        },
                        media_type,
                        size_bytes: data.len() * 3 / 4, // Approximate
                    };
                    serde_json::to_string_pretty(&info).unwrap_or_default()
                })
            }
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "Invalid source: {}. Must be 'url' or 'file'",
                    source
                )))
            }
        };

        match result {
            Ok(content) => Ok(ToolResult::success(
                "fetch_image".to_string(),
                "fetch_image".to_string(),
                content,
            )),
            Err(e) => Ok(ToolResult::error(
                "fetch_image".to_string(),
                "fetch_image".to_string(),
                e.to_string(),
            )),
        }
    }
}

impl Default for ImageTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        let data = b"Hello";
        let encoded = base64_encode(data);
        assert_eq!(encoded, "SGVsbG8=");
    }

    #[test]
    fn test_tool_parameters() {
        let tool = ImageTool::new();
        let params = tool.parameters();

        assert!(params.get("properties").is_some());
        assert!(params["properties"].get("source").is_some());
    }
}
