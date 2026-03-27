//! AGI Agent - Main Entry Point
//! 
//! Implements the agent as specified in SPEC.md

mod agent;
mod api;
mod config;
mod memory;
mod models;

use anyhow::Context;

use agent::{Agent, AgentConfig};
use api::chat::AppState;
use memory::{EmbeddingClient, SqliteMemoryStore};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Health response
#[derive(serde::Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
}

/// Create the Axum router with all endpoints
fn create_router(state: Arc<AppState>) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    axum::Router::new()
        // Health
        .route("/health", axum::routing::get(health_handler))
        // Models
        .route("/v1/models", axum::routing::get(api::models_handler))
        // Chat completions
        .route("/v1/chat/completions", axum::routing::post(api::chat_handler))
        // Memory endpoints
        .route("/memories", axum::routing::get(api::list_memories))
        .route("/memories", axum::routing::post(api::store_memory))
        .route("/memories/query", axum::routing::post(api::query_memory))
        .route("/memories/stats", axum::routing::get(api::memory_stats))
        .route("/memories/{id}", axum::routing::delete(api::delete_memory))
        // Training endpoints
        .route("/training/trigger", axum::routing::post(api::trigger_training))
        .route("/training/status", axum::routing::get(api::get_training_status))
        .route("/training/models", axum::routing::get(api::list_models))
        .route("/training/cancel", axum::routing::post(api::cancel_training))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

async fn health_handler() -> axum::Json<HealthResponse> {
    axum::Json(HealthResponse {
        status: "ok".to_string(),
        service: "agi-agent".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agi_agent=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting AGI Agent v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = config::AppConfig::load()?;
    tracing::info!("Configuration loaded");

    // Create memory store
    let memory_path = config.memory.storage_path.join("memories.db");
    let index_path = config.memory.storage_path.join("index");
    
    let memory_store = Arc::new(
        SqliteMemoryStore::new(&memory_path, &index_path)
            .context("Failed to create memory store")?,
    );
    tracing::info!("Memory store initialized at {:?}", memory_path);

    // Create embedding client
    let embedding_client = Arc::new(EmbeddingClient::new(memory::embedding::EmbeddingClientConfig {
        base_url: config.model.base_url.clone(),
        model: config.model.embedding_model.clone(),
        dimensions: config.model.embedding_dim,
        batch_size: 32,
        api_key: config.model.api_key.clone(),
    }));
    tracing::info!("Embedding client initialized");

    // Create agent
    let agent_config = AgentConfig {
        system_prompt: AgentConfig::default_system_prompt(),
        max_context_length: config.model.max_tokens,
        enable_reasoning: true,
        reasoning_depth: 3,
        enable_memory: true,
        enable_tools: false,
        max_tool_calls: 10,
    };

    let agent = Agent::new(config.clone(), agent_config)
        .context("Failed to create agent")?;
    tracing::info!("Agent initialized");

    // Create application state
    let state = Arc::new(AppState {
        agent: Arc::new(tokio::sync::RwLock::new(Some(agent))),
        memory_store,
        embedding_client,
    });

    // Create router
    let app = create_router(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Starting HTTP server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to bind to address")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for Ctrl+C");
            tracing::info!("Shutting down...");
        })
        .await
        .context("Server error")?;

    Ok(())
}
