//! Knowledge API endpoints
//! 
//! External knowledge base integration endpoints

use crate::api::chat::AppState;
use crate::knowledge::{KnowledgeEntry, KnowledgeSource};
use axum::{
    extract::{Path, State, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Query parameters for knowledge search
#[derive(Debug, Deserialize)]
pub struct KnowledgeQuery {
    pub q: String,
    pub source: Option<String>,
    pub limit: Option<usize>,
}

/// Response for knowledge search
#[derive(Debug, Serialize)]
pub struct KnowledgeSearchResponse {
    pub query: String,
    pub source: String,
    pub results: Vec<KnowledgeEntry>,
    pub count: usize,
}

/// Search external knowledge sources
pub async fn search_knowledge(
    Query(params): Query<KnowledgeQuery>,
    State(state): State<Arc<AppState>>,
) -> Json<KnowledgeSearchResponse> {
    let limit = params.limit.unwrap_or(5);
    let source = params.source.as_deref().unwrap_or("all");
    let mut results = Vec::new();

    // Search based on requested source
    match source {
        "wikipedia" if state.knowledge_config.wikipedia_enabled => {
            if let Ok(search_results) = state.wikipedia.search(&params.q).await {
                for result in search_results.into_iter().take(limit) {
                    results.push(KnowledgeEntry::new(
                        KnowledgeSource::Wikipedia,
                        result.title,
                        result.snippet,
                    ));
                }
            }
        }
        "arxiv" if state.knowledge_config.arxiv_enabled => {
            if let Ok(papers) = state.arxiv.search(&params.q, limit).await {
                for paper in papers {
                    let entry = state.arxiv.to_knowledge_entry(&paper);
                    results.push(entry);
                }
            }
        }
        "web" if state.knowledge_config.web_fetch_enabled => {
            // Web search would need integration with search service
            // For now, just return a placeholder
        }
        _ => {
            // Search all sources
            if state.knowledge_config.wikipedia_enabled {
                if let Ok(search_results) = state.wikipedia.search(&params.q).await {
                    for result in search_results.into_iter().take(limit) {
                        results.push(KnowledgeEntry::new(
                            KnowledgeSource::Wikipedia,
                            result.title,
                            result.snippet,
                        ));
                    }
                }
            }
        }
    }

    let count = results.len();
    Json(KnowledgeSearchResponse {
        query: params.q,
        source: source.to_string(),
        results,
        count,
    })
}

/// Wikipedia article endpoint
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct WikipediaParams {
    pub title: String,
}

/// Get Wikipedia article content
pub async fn get_wikipedia_article(
    Path(title): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<KnowledgeEntry>, String> {
    state
        .wikipedia
        .get_article(&title)
        .await
        .map(Json)
        .map_err(|e| e.to_string())
}

/// ArXiv paper endpoint
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ArxivParams {
    pub id: String,
}

/// Get ArXiv paper content
pub async fn get_arxiv_paper(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<KnowledgeEntry>, String> {
    state
        .arxiv
        .get_paper(&id)
        .await
        .map(|paper| {
            let entry = state.arxiv.to_knowledge_entry(&paper);
            Json(entry)
        })
        .map_err(|e| e.to_string())
}

/// Fetch URL endpoint
#[derive(Debug, Deserialize)]
pub struct FetchParams {
    pub url: String,
    pub article: Option<bool>,
}

/// Fetch web content from URL
pub async fn fetch_url(
    Query(params): Query<FetchParams>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<KnowledgeEntry>, String> {
    let fetcher = state.web_fetcher.clone();
    let url = params.url.clone();
    
    let entry = if params.article.unwrap_or(false) {
        fetcher.fetch_article(&url).await
    } else {
        fetcher.fetch(&url).await
    };

    entry.map(Json).map_err(|e| e.to_string())
}

/// Knowledge sources status
#[derive(Debug, Serialize)]
pub struct KnowledgeSourcesStatus {
    pub wikipedia_enabled: bool,
    pub arxiv_enabled: bool,
    pub web_fetch_enabled: bool,
}

/// Get knowledge sources status
pub async fn knowledge_sources_status(
    State(state): State<Arc<AppState>>,
) -> Json<KnowledgeSourcesStatus> {
    Json(KnowledgeSourcesStatus {
        wikipedia_enabled: state.knowledge_config.wikipedia_enabled,
        arxiv_enabled: state.knowledge_config.arxiv_enabled,
        web_fetch_enabled: state.knowledge_config.web_fetch_enabled,
    })
}
