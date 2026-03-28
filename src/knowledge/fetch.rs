//! Web Fetcher
//!
//! Provides general web content fetching with HTML parsing and text extraction

use super::{KnowledgeEntry, KnowledgeSource};
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

/// Web fetcher for retrieving and parsing web content
#[derive(Clone)]
pub struct WebFetcher {
    client: Client,
    #[allow(dead_code)]
    timeout_seconds: u64,
    max_content_length: usize,
}

impl WebFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("Mozilla/5.0 (compatible; AGI-Agent/1.0)")
                .build()
                .unwrap_or_default(),
            timeout_seconds: 30,
            max_content_length: 50000,
        }
    }

    pub fn with_config(timeout_seconds: u64, max_content_length: usize) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(timeout_seconds))
                .user_agent("Mozilla/5.0 (compatible; AGI-Agent/1.0)")
                .build()
                .unwrap_or_default(),
            timeout_seconds,
            max_content_length,
        }
    }

    /// Fetch and parse a URL
    pub async fn fetch(&self, url: &str) -> Result<KnowledgeEntry> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Fetch failed: {} - {}", url, response.status());
        }

        // Get content type first before consuming response
        let content_type: String = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        let body = response.text().await?;

        // Extract content based on content type
        let (title, content) = if content_type.contains("html") {
            self.parse_html(&body)
        } else {
            self.parse_plain_text(&body)
        };

        let entry = KnowledgeEntry::new(
            KnowledgeSource::Web,
            title.unwrap_or_else(|| "Web Page".to_string()),
            content,
        )
        .with_url(url.to_string());

        Ok(entry)
    }

    /// Fetch and extract just the main content (article text)
    pub async fn fetch_article(&self, url: &str) -> Result<KnowledgeEntry> {
        let entry = self.fetch(url).await?;

        // Try to extract more focused content
        let content = self.extract_article_content(&entry.content);

        let mut entry = entry;
        entry.content = content;

        Ok(entry)
    }

    /// Parse HTML content
    fn parse_html(&self, html: &str) -> (Option<String>, String) {
        let title = self.extract_title(html);
        let content = self.extract_content(html);
        (title, content)
    }

    /// Parse plain text content
    fn parse_plain_text(&self, text: &str) -> (Option<String>, String) {
        (None, text.chars().take(self.max_content_length).collect())
    }

    /// Extract title from HTML
    fn extract_title(&self, html: &str) -> Option<String> {
        // Try <title> tag first
        if let Some(start) = html.find("<title>") {
            let start = start + 7;
            if let Some(end) = html[start..].find("</title>") {
                let title = html[start..start + end].trim().to_string();
                if !title.is_empty() {
                    return Some(self.decode_html_entities(&title));
                }
            }
        }

        // Try <h1> tag
        if let Some(start) = html.find("<h1") {
            if let Some(content_start) = html[start..].find('>') {
                let content_start = start + content_start + 1;
                if let Some(end) = html[content_start..].find("</h1>") {
                    let h1 = html[content_start..content_start + end].trim().to_string();
                    if !h1.is_empty() {
                        return Some(self.decode_html_entities(&h1));
                    }
                }
            }
        }

        // Try og:title meta tag
        if let Some(start) = html.find("property=\"og:title\"") {
            if let Some(content_start) = html[start..].find("content=\"") {
                let content_start = start + content_start + 9;
                if let Some(content_end) = html[content_start..].find('"') {
                    return Some(html[content_start..content_start + content_end].to_string());
                }
            }
        }

        None
    }

    /// Extract main content from HTML
    fn extract_content(&self, html: &str) -> String {
        let mut result = String::new();
        let mut in_script = false;
        let mut in_style = false;
        let mut in_tag = false;
        let mut text_buffer = String::new();

        let chars: Vec<char> = html.chars().collect();
        let mut i = 0;

        while i < chars.len() && result.len() < self.max_content_length {
            let c = chars[i];

            if c == '<' {
                in_tag = true;

                // Check for script/style tags
                let remaining: String = chars[i..].iter().take(10).collect();
                let lower = remaining.to_lowercase();

                if lower.starts_with("<script") {
                    in_script = true;
                } else if lower.starts_with("<style") {
                    in_style = true;
                } else if lower.starts_with("</script") {
                    in_script = false;
                } else if lower.starts_with("</style") {
                    in_style = false;
                }

                // Flush text buffer
                if !text_buffer.is_empty() {
                    let trimmed = text_buffer.trim();
                    if !trimmed.is_empty() {
                        result.push_str(trimmed);
                        result.push('\n');
                    }
                    text_buffer.clear();
                }
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag && !in_script && !in_style {
                text_buffer.push(c);
            }

            i += 1;
        }

        // Flush remaining text
        if !text_buffer.is_empty() {
            let trimmed = text_buffer.trim();
            if !trimmed.is_empty() {
                result.push_str(trimmed);
            }
        }

        // Clean up the result
        self.clean_text(&result)
    }

    /// Extract article-specific content (paragraphs)
    fn extract_article_content(&self, text: &str) -> String {
        // Simple heuristic: extract longer paragraphs
        text.lines()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.len() > 50 && trimmed.len() < 500
            })
            .take(20)
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Clean extracted text
    fn clean_text(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                line.chars()
                    .filter(|c| !c.is_control())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Decode common HTML entities
    fn decode_html_entities(&self, text: &str) -> String {
        text.replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&nbsp;", " ")
            .replace("&mdash;", "—")
            .replace("&ndash;", "–")
            .replace("&hellip;", "…")
    }

    /// Fetch multiple URLs in parallel
    pub async fn fetch_multiple(&self, urls: &[String]) -> Vec<Result<KnowledgeEntry>> {
        let futures = urls.iter().map(|url| self.fetch(url));
        futures::future::join_all(futures).await
    }
}

impl Default for WebFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let fetcher = WebFetcher::new();
        
        let html = r#"<html>
<head><title>Test Page Title</title></head>
<body>Content</body>
</html>"#;
        
        let title = fetcher.extract_title(html);
        assert_eq!(title, Some("Test Page Title".to_string()));
    }

    #[test]
    fn test_extract_title_from_og() {
        let fetcher = WebFetcher::new();
        
        let html = r#"<html>
<head>
<meta property="og:title" content="OG Title">
</head>
<body>Content</body>
</html>"#;
        
        let title = fetcher.extract_title(html);
        assert_eq!(title, Some("OG Title".to_string()));
    }

    #[test]
    fn test_clean_text() {
        let fetcher = WebFetcher::new();
        
        let input = "Line 1\n\nLine 2\n   \nLine 3";
        let cleaned = fetcher.clean_text(input);
        
        assert!(cleaned.contains("Line 1"));
        assert!(cleaned.contains("Line 2"));
        assert!(cleaned.contains("Line 3"));
    }

    #[test]
    fn test_decode_html_entities() {
        let fetcher = WebFetcher::new();
        
        let input = "Hello &amp; World &lt;3&gt;";
        let decoded = fetcher.decode_html_entities(input);
        
        assert_eq!(decoded, "Hello & World <3>");
    }

    #[test]
    fn test_extract_article_content() {
        let fetcher = WebFetcher::new();
        
        let text = "Short line\n\nThis is a longer paragraph that should be extracted because it has more than 50 characters.\n\nAnother short line\n\nYet another medium length paragraph with sufficient content for extraction.";
        
        let articles = fetcher.extract_article_content(text);
        
        assert!(articles.contains("longer paragraph"));
        assert!(!articles.contains("Short line"));
    }

    #[tokio::test]
    async fn test_fetch_multiple_empty() {
        let fetcher = WebFetcher::new();
        let results = fetcher.fetch_multiple(&[]).await;
        assert!(results.is_empty());
    }
}
