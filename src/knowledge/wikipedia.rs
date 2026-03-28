//! Wikipedia API Client
//!
//! Provides access to Wikipedia for retrieving factual information

use super::{KnowledgeEntry, KnowledgeSource};
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

/// Wikipedia API client
#[derive(Clone)]
pub struct WikipediaClient {
    client: Client,
    base_url: String,
    #[allow(dead_code)]
    language: String,
}

impl WikipediaClient {
    pub fn new() -> Self {
        Self::with_language("en")
    }

    pub fn with_language(language: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("AGI-Agent/1.0 (Rust)")
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: format!("https://{}.wikipedia.org", language),
            language: language.to_string(),
        }
    }

    /// Search Wikipedia for articles
    pub async fn search(&self, query: &str) -> Result<Vec<WikipediaSearchResult>> {
        let url = format!("{}/w/api.php", self.base_url);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("action", "query"),
                ("format", "json"),
                ("list", "search"),
                ("srsearch", query),
                ("srlimit", "10"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Wikipedia search failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct WikipediaResponse {
            query: Option<QueryResult>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct QueryResult {
            search: Vec<SearchItem>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SearchItem {
            title: String,
            snippet: String,
            #[allow(dead_code)]
            page_id: u64,
        }

        let result: WikipediaResponse = response.json().await?;

        Ok(result
            .query
            .map(|q| {
                q.search
                    .into_iter()
                    .map(|s| WikipediaSearchResult {
                        title: s.title,
                        snippet: strip_wiki_tags(&s.snippet),
                        page_id: s.page_id,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Get article content by title
    pub async fn get_article(&self, title: &str) -> Result<KnowledgeEntry> {
        let url = format!("{}/w/api.php", self.base_url);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("action", "query"),
                ("format", "json"),
                ("titles", title),
                ("prop", "extracts"),
                ("explaintext", "true"),
                ("exintro", "false"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Wikipedia article fetch failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct WikipediaResponse {
            query: Option<QueryResult>,
        }

        #[derive(Deserialize)]
        struct QueryResult {
            pages: std::collections::HashMap<String, PageResult>,
        }

        #[derive(Deserialize)]
        struct PageResult {
            title: String,
            extract: Option<String>,
            #[allow(dead_code)]
            page_id: u64,
        }

        let result: WikipediaResponse = response.json().await?;

        let pages = result.query.map(|q| q.pages).unwrap_or_default();
        
        // Find the page (may be in negative ID format for missing pages)
        for (_key, page) in pages {
            if !page.title.starts_with("List of") && page.extract.is_some() {
                let url = format!("{}/wiki/{}", self.base_url, url_encode(&page.title));
                return Ok(KnowledgeEntry::new(
                    KnowledgeSource::Wikipedia,
                    page.title.clone(),
                    page.extract.unwrap_or_default(),
                )
                .with_url(url));
            }
        }

        anyhow::bail!("Article not found: {}", title)
    }
}

impl Default for WikipediaClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Search result from Wikipedia
#[derive(Debug, Clone)]
pub struct WikipediaSearchResult {
    pub title: String,
    pub snippet: String,
    #[allow(dead_code)]
    pub page_id: u64,
}

/// Strip Wikipedia formatting tags from text
fn strip_wiki_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }

    // Clean up common HTML entities
    result
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// URL encode a string
fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '/' => "%2F".to_string(),
            '#' => "%23".to_string(),
            _ if c.is_ascii_alphanumeric() || c == '-' || c == '_' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_wiki_tags() {
        let input = "This is <span class=\"searchmatch\">highlighted</span> text";
        let output = strip_wiki_tags(input);
        assert!(output.contains("highlighted"));
        assert!(!output.contains("<span"));
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("Rust Programming"), "Rust%20Programming");
        assert_eq!(url_encode("Machine Learning/LLM"), "Machine%20Learning%2FLLM");
    }

    #[tokio::test]
    async fn test_wikipedia_client_creation() {
        let client = WikipediaClient::new();
        assert_eq!(client.language, "en");
    }

    #[tokio::test]
    async fn test_wikipedia_client_custom_language() {
        let client = WikipediaClient::with_language("de");
        assert_eq!(client.language, "de");
    }
}
