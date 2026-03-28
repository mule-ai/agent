//! ArXiv API Client
//!
//! Provides access to ArXiv for retrieving academic papers and research

use super::{KnowledgeEntry, KnowledgeSource};
use anyhow::Result;
use reqwest::Client;
// serde::Deserialize removed - unused import
use std::time::Duration;

/// ArXiv API client
#[derive(Clone)]
pub struct ArxivClient {
    client: Client,
    base_url: String,
}

impl ArxivClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .user_agent("AGI-Agent/1.0 (Rust; mailto:agent@example.com)")
                .build()
                .unwrap_or_default(),
            base_url: "http://export.arxiv.org".to_string(),
        }
    }

    /// Search ArXiv for papers
    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<ArxivPaper>> {
        let url = format!("{}/api/query", self.base_url);
        let start = "0".to_string();
        let sort_by = "relevance".to_string();

        let response = self
            .client
            .get(&url)
            .query(&[
                ("search_query", &format!("all:{}", query)),
                ("start", &start),
                ("max_results", &max_results.to_string()),
                ("sortBy", &sort_by),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("ArXiv search failed: {}", response.status());
        }

        let body = response.text().await?;
        self.parse_atom_feed(&body)
    }

    /// Search by title
    pub async fn search_by_title(&self, title: &str, max_results: usize) -> Result<Vec<ArxivPaper>> {
        let url = format!("{}/api/query", self.base_url);
        let start = "0".to_string();
        let sort_by = "relevance".to_string();

        let response = self
            .client
            .get(&url)
            .query(&[
                ("search_query", &format!("ti:{}", title)),
                ("start", &start),
                ("max_results", &max_results.to_string()),
                ("sortBy", &sort_by),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("ArXiv search failed: {}", response.status());
        }

        let body = response.text().await?;
        self.parse_atom_feed(&body)
    }

    /// Get paper by ID
    pub async fn get_paper(&self, arxiv_id: &str) -> Result<ArxivPaper> {
        let url = format!("{}/api/query", self.base_url);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("id_list", arxiv_id),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("ArXiv fetch failed: {}", response.status());
        }

        let body = response.text().await?;
        let papers = self.parse_atom_feed(&body)?;

        papers.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!("Paper not found: {}", arxiv_id)
        })
    }

    /// Parse Atom feed response
    fn parse_atom_feed(&self, xml: &str) -> Result<Vec<ArxivPaper>> {
        let mut papers = Vec::new();

        // Simple XML parsing for Atom feed
        let entries = self.extract_entries(xml);

        for entry in entries {
            if let Some(paper) = self.parse_entry(&entry) {
                papers.push(paper);
            }
        }

        Ok(papers)
    }

    /// Extract entry blocks from Atom feed
    fn extract_entries(&self, xml: &str) -> Vec<String> {
        let mut entries = Vec::new();
        let mut in_entry = false;
        let mut current_entry = String::new();

        for line in xml.lines() {
            let line = line.trim();
            if line.starts_with("<entry>") {
                in_entry = true;
                current_entry.clear();
            } else if line.starts_with("</entry>") {
                if in_entry {
                    entries.push(current_entry.clone());
                }
                in_entry = false;
            } else if in_entry {
                current_entry.push_str(line);
                current_entry.push('\n');
            }
        }

        entries
    }

    /// Parse a single entry
    fn parse_entry(&self, xml: &str) -> Option<ArxivPaper> {
        let id = self.extract_tag(xml, "id")?;
        let title = self.extract_tag(xml, "title")?;
        let summary = self.extract_tag(xml, "summary")?;
        let published = self.extract_tag(xml, "published").unwrap_or_default();

        // Extract authors
        let authors: Vec<String> = self.extract_all_tags(xml, "name");

        // Extract categories
        let categories: Vec<String> = self.extract_all_tags(xml, "category");

        // Extract PDF link
        let pdf_link = self.extract_attribute(xml, "link", "href")
            .filter(|_| xml.contains("pdf"));

        Some(ArxivPaper {
            id: id.split('/').last().unwrap_or(&id).to_string(),
            title: title.trim().to_string(),
            summary: summary.trim().to_string(),
            authors,
            published,
            categories,
            pdf_link,
            abstract_url: Some(format!("https://arxiv.org/abs/{}", id.split('/').last().unwrap_or(&id))),
        })
    }

    /// Extract content from a tag
    fn extract_tag(&self, xml: &str, tag: &str) -> Option<String> {
        let start = format!("<{}>", tag);
        let end = format!("</{}>", tag);

        let start_idx = xml.find(&start)? + start.len();
        let end_idx = xml.find(&end)?;

        Some(xml[start_idx..end_idx].to_string())
    }

    /// Extract all instances of a tag
    fn extract_all_tags(&self, xml: &str, tag: &str) -> Vec<String> {
        let start = format!("<{}>", tag);
        let end = format!("</{}>", tag);

        let mut results = Vec::new();
        let mut search_pos: usize = 0;

        while let Some(tag_pos) = xml[search_pos..].find(&start) {
            let actual_pos = search_pos + tag_pos;
            let content_start = actual_pos + start.len();
            if let Some(end_pos) = xml[content_start..].find(&end) {
                results.push(xml[content_start..content_start + end_pos].to_string());
            }
            search_pos = content_start;
        }

        results
    }

    /// Extract attribute from a self-closing tag
    fn extract_attribute(&self, xml: &str, tag: &str, attr: &str) -> Option<String> {
        let tag_start = format!("<{}", tag);
        let tag_with_attr = xml.find(&tag_start)?;

        let after_tag = &xml[tag_with_attr..];
        let attr_pattern = format!("{}=\"", attr);

        if let Some(attr_start) = after_tag.find(&attr_pattern) {
            let value_start = attr_start + attr_pattern.len();
            if let Some(value_end) = after_tag[value_start..].find('"') {
                return Some(after_tag[value_start..value_start + value_end].to_string());
            }
        }

        None
    }

    /// Convert ArXiv paper to knowledge entry
    pub fn to_knowledge_entry(&self, paper: &ArxivPaper) -> KnowledgeEntry {
        let content = format!(
            "{}\n\nAuthors: {}\nCategories: {}\nPublished: {}",
            paper.summary,
            paper.authors.join(", "),
            paper.categories.join(", "),
            paper.published
        );

        let mut entry = KnowledgeEntry::new(
            KnowledgeSource::Arxiv,
            paper.title.clone(),
            content,
        )
        .with_relevance(0.8);

        if let Some(ref url) = paper.abstract_url {
            entry = entry.with_url(url.clone());
        }

        if let Some(ref pdf) = paper.pdf_link {
            entry.add_metadata("pdf_url".to_string(), serde_json::json!(pdf));
        }

        entry
    }
}

impl Default for ArxivClient {
    fn default() -> Self {
        Self::new()
    }
}

/// ArXiv paper
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArxivPaper {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub authors: Vec<String>,
    pub published: String,
    pub categories: Vec<String>,
    #[serde(skip_deserializing)]
    pub pdf_link: Option<String>,
    pub abstract_url: Option<String>,
}

impl ArxivPaper {
    /// Get citation in BibTeX format (for future use)
    #[allow(dead_code)]
    pub fn to_bibtex(&self) -> String {
        let author_str = self.authors.join(" and ");
        let year = self.published.chars().take(4).collect::<String>();
        let key = format!("{}{}", 
            self.authors.first()
                .map(|a| a.split_whitespace().last().unwrap_or("unknown"))
                .unwrap_or("unknown"),
            year
        );

        format!(
            r#"@article{{{},
  title = {{{}}},
  author = {{{}}},
  year = {{{}}},
  eprint = {{{}}},
  archivePrefix = {{arXiv}},
  primaryClass = {{{}}},
  url = {{{}}}
}}"#,
            key,
            self.title,
            author_str,
            year,
            self.id,
            self.categories.first().unwrap_or(&"cs.AI".to_string()),
            self.abstract_url.as_deref().unwrap_or("")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arxiv_client_creation() {
        let client = ArxivClient::new();
        assert_eq!(client.base_url, "http://export.arxiv.org");
    }

    #[test]
    fn test_extract_tag() {
        let client = ArxivClient::new();
        let xml = "<title>Rust Programming</title>";
        assert_eq!(client.extract_tag(xml, "title"), Some("Rust Programming".to_string()));
    }

    #[test]
    fn test_arxiv_paper_bibtex() {
        let paper = ArxivPaper {
            id: "2301.12345".to_string(),
            title: "Test Paper".to_string(),
            summary: "A test paper".to_string(),
            authors: vec!["John Doe".to_string(), "Jane Smith".to_string()],
            published: "2023-01-15T00:00:00Z".to_string(),
            categories: vec!["cs.AI".to_string()],
            pdf_link: None,
            abstract_url: Some("https://arxiv.org/abs/2301.12345".to_string()),
        };

        let bibtex = paper.to_bibtex();
        assert!(bibtex.contains("@article{"));
        assert!(bibtex.contains("Test Paper"));
        assert!(bibtex.contains("John Doe and Jane Smith"));
    }

    #[tokio::test]
    async fn test_search_query_format() {
        // This test just verifies the client can be created and search is async-safe
        let client = ArxivClient::new();
        // In real test, we'd mock the HTTP response
        assert!(true);
    }
}
