//! AGI Agent - Main Entry Point
//! 
//! Implements the agent as specified in SPEC.md

mod agent;
mod api;
mod cli;
mod config;
mod knowledge;
mod memory;
mod models;
mod services;
mod tools;
mod training;

use anyhow::Context;

use agent::{Agent, AgentConfig, SessionStore};
use api::chat::AppState;
use memory::{EmbeddingClient, SqliteMemoryStore};
use services::{CuriosityEngine, MemoryEvictionService, SearchLearningService, ServiceManager, SessionReviewService, OnlineLearningService, SelfImproveEngine, TheoryOfMindEngine, BatchTrainingService, SchedulerService, SchedulerConfig};
use std::sync::Arc;
use tools::ToolRegistry;
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
        // Batch training endpoints
        .route("/training/batch/status", axum::routing::get(api::batch_training_status))
        .route("/training/batch/collect", axum::routing::post(api::collect_training_examples))
        .route("/training/batch/add", axum::routing::post(api::add_training_example))
        .route("/training/batch/stats", axum::routing::get(api::get_accumulator_stats))
        .route("/training/batch/run", axum::routing::post(api::run_batch_training))
        .route("/training/batch/export", axum::routing::get(api::export_training_examples))
        .route("/training/batch/clear", axum::routing::post(api::clear_accumulator))
        // Quality filtering and model registry endpoints
        .route("/training/batch/filter", axum::routing::post(api::filter_examples_by_quality))
        .route("/training/models/list", axum::routing::get(api::list_trained_models))
        .route("/training/models/current", axum::routing::get(api::list_trained_models))
        .route("/training/models/current", axum::routing::post(api::set_current_model))
        // Model management (hot-swap)
        .route("/model/status", axum::routing::get(api::get_model_status))
        .route("/model/update", axum::routing::post(api::update_model))
        .route("/model/validate", axum::routing::post(api::validate_model))
        .route("/model/available", axum::routing::get(api::list_available_models))
        // Learned concepts
        .route("/concepts", axum::routing::get(api::get_learned_concepts))
        .route("/concepts/search", axum::routing::post(api::search_learned_concepts))
        // Service endpoints
        .route("/services/status", axum::routing::get(api::services_status))
        .route("/services/session-review", axum::routing::post(api::run_session_review))
        .route("/services/eviction", axum::routing::post(api::run_eviction))
        .route("/services/search-learning", axum::routing::post(api::run_search_learning))
        // Session endpoints (Phase 2)
        .route("/sessions", axum::routing::get(api::list_sessions))
        .route("/sessions", axum::routing::post(api::create_session))
        .route("/sessions/{id}", axum::routing::get(api::get_session))
        .route("/sessions/{id}", axum::routing::delete(api::delete_session))
        .route("/sessions/{id}/end", axum::routing::post(api::end_session))
        // Knowledge endpoints (Phase 2 - External Knowledge Base)
        .route("/knowledge/search", axum::routing::get(api::search_knowledge))
        .route("/knowledge/wikipedia/{title}", axum::routing::get(api::get_wikipedia_article))
        .route("/knowledge/arxiv/{id}", axum::routing::get(api::get_arxiv_paper))
        .route("/knowledge/fetch", axum::routing::get(api::fetch_url))
        .route("/knowledge/sources", axum::routing::get(api::knowledge_sources_status))
        // Curiosity Engine endpoints (Phase 3)
        .route("/curiosity/stats", axum::routing::get(api::curiosity_stats))
        .route("/curiosity/detect", axum::routing::post(api::detect_gaps))
        .route("/curiosity/gaps", axum::routing::get(api::list_gaps))
        .route("/curiosity/gaps/pending", axum::routing::get(api::pending_gaps))
        .route("/curiosity/explore", axum::routing::post(api::explore_gap))
        .route("/curiosity/process", axum::routing::post(api::process_curiosity_queue))
        .route("/curiosity/dismiss", axum::routing::post(api::dismiss_gap))
        // Online Learning endpoints (Phase 3 - Continuous RL)
        .route("/learning/stats", axum::routing::get(api::learning_stats))
        .route("/learning/buffer", axum::routing::get(api::learning_buffer_stats))
        .route("/learning/learn", axum::routing::post(api::run_online_learn))
        .route("/learning/concepts", axum::routing::get(api::learning_concepts))
        .route("/learning/example", axum::routing::post(api::add_learning_example))
        .route("/learning/session", axum::routing::post(api::add_session_experiences))
        .route("/learning/prune", axum::routing::post(api::prune_learning_buffer))
        // Self-Improvement endpoints (Phase 3)
        .route("/self-improve/stats", axum::routing::get(api::self_improve_stats))
        .route("/self-improve/extended-stats", axum::routing::get(api::self_improve_extended_stats))
        .route("/self-improve/analyze", axum::routing::post(api::run_self_improve))
        .route("/self-improve/improvements", axum::routing::get(api::get_improvements))
        .route("/self-improve/improvements", axum::routing::post(api::get_improvements))
        .route("/self-improve/apply", axum::routing::post(api::apply_improvement))
        .route("/self-improve/reject", axum::routing::post(api::reject_improvement))
        .route("/self-improve/rollback", axum::routing::post(api::rollback_improvement))
        .route("/self-improve/prompt", axum::routing::get(api::get_system_prompt))
        .route("/self-improve/prompt", axum::routing::post(api::update_system_prompt))
        // Code Analysis endpoints (Self-Improvement)
        .route("/self-improve/code/analyze", axum::routing::post(api::analyze_code_from_search))
        .route("/self-improve/code/patterns", axum::routing::get(api::get_code_patterns))
        .route("/self-improve/code/improvements", axum::routing::get(api::get_code_improvements))
        .route("/self-improve/code/apply", axum::routing::post(api::apply_code_improvement))
        .route("/self-improve/code/rollback", axum::routing::post(api::rollback_code_improvement))
        .route("/self-improve/code/history", axum::routing::get(api::get_improvement_history))
        // Theory of Mind endpoints (Phase 3)
        .route("/tom/stats", axum::routing::get(api::tom_stats))
        .route("/tom/user", axum::routing::post(api::update_user_model))
        .route("/tom/user", axum::routing::get(api::get_user_model))
        .route("/tom/users", axum::routing::get(api::get_all_user_models))
        .route("/tom/analyze", axum::routing::post(api::analyze_for_response))
        .route("/tom/history", axum::routing::get(api::get_conversation_history))
        .route("/tom/clear", axum::routing::post(api::clear_user_model))
        .route("/tom/trust", axum::routing::post(api::update_trust))
        .route("/tom/intention", axum::routing::post(api::satisfy_intention))
        // Scheduler endpoints (TASK 5)
        .route("/scheduler/stats", axum::routing::get(api::scheduler_stats))
        .route("/scheduler/trigger", axum::routing::post(api::scheduler_trigger_training))
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

    // Create tool registry
    let tool_registry = Arc::new(ToolRegistry::default_registry());
    tracing::info!("Tool registry initialized with {} tools", tool_registry.list_tools().len());

    // Create batch training service first (so search learning can reference it)
    let batch_training_service = Arc::new(BatchTrainingService::new(config.training.clone()));
    
    // Create other background services first (needed for scheduler)
    let service_manager = Arc::new(ServiceManager::new());
    let session_review_service = Arc::new(SessionReviewService::new());
    let memory_eviction_service = Arc::new(MemoryEvictionService::new());
    
    // Create scheduler service for automated tasks with all services wired up
    let scheduler_config = crate::services::SchedulerConfig {
        enabled: config.scheduler.enabled,
        batch_training_enabled: config.scheduler.batch_training_enabled,
        batch_training_schedule: config.scheduler.batch_training_schedule.clone(),
        memory_eviction_enabled: config.scheduler.memory_eviction_enabled,
        memory_eviction_schedule: config.scheduler.memory_eviction_schedule.clone(),
        session_review_enabled: config.scheduler.session_review_enabled,
        session_review_schedule: config.scheduler.session_review_schedule.clone(),
    };
    let scheduler_service = Arc::new(SchedulerService::with_services(
        scheduler_config,
        batch_training_service.clone(),
        memory_eviction_service.clone(),
        session_review_service.clone(),
    ));
    
    // Create search learning service and wire it to batch training
    let search_learning_service = Arc::new(SearchLearningService::new());
    
    // Wire search learning to batch training service
    let batch_service_clone = (*batch_training_service).clone();
    tokio::spawn(async move {
        // Wait briefly for services to be ready
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        search_learning_service.set_batch_training_service(batch_service_clone).await;
    });
    
    let curiosity_engine = Arc::new(CuriosityEngine::new());
    let online_learning_service = Arc::new(OnlineLearningService::new());
    let self_improve_engine = Arc::new(SelfImproveEngine::new(Default::default()));
    let theory_of_mind_engine = Arc::new(TheoryOfMindEngine::new(Default::default()));
    
    // Wire curiosity engine to batch training service for automatic training data generation
    let batch_service_for_curiosity = (*batch_training_service).clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        curiosity_engine.wire_to_batch_training(batch_service_for_curiosity).await;
    });
    
    tracing::info!("Background services initialized (Search Learning v0.1, Batch Training v0.1, Scheduler v0.2, Curiosity Engine v0.1, Online Learning v0.1, Self-Improve v0.1, Theory of Mind v0.1)");

    // Create agent
    let agent_config = AgentConfig {
        system_prompt: AgentConfig::default_system_prompt(),
        max_context_length: config.model.max_tokens,
        enable_reasoning: true,
        reasoning_depth: 3,
        enable_memory: true,
        enable_tools: true,
        max_tool_calls: 10,
    };

    let agent = Agent::new(
        config.clone(), 
        agent_config,
        memory_store.clone(),
        embedding_client.clone(),
        tool_registry.clone(),
    )
    .context("Failed to create agent")?;
    tracing::info!("Agent initialized");

    // Create session store for persistent sessions (Phase 2 feature)
    let session_store = match SessionStore::new(config.memory.storage_path.join("sessions.db")) {
        Ok(store) => {
            tracing::info!("Session store initialized for persistent sessions");
            Some(Arc::new(store))
        }
        Err(e) => {
            tracing::warn!("Failed to create session store, using in-memory only: {}", e);
            None
        }
    };

    // Create knowledge clients for external knowledge base
    let wikipedia = knowledge::WikipediaClient::new();
    let arxiv = knowledge::ArxivClient::new();
    let web_fetcher = knowledge::WebFetcher::new();
    let knowledge_config = knowledge::KnowledgeConfig::default();

    // Create application state
    let state = Arc::new(AppState {
        agent: Arc::new(tokio::sync::RwLock::new(Some(agent))),
        memory_store,
        embedding_client,
        tool_registry,
        service_manager,
        session_review_service,
        memory_eviction_service,
        search_learning_service,
        curiosity_engine,
        online_learning_service,
        self_improve_engine,
        theory_of_mind_engine,
        batch_training_service,
        scheduler_service,
        session_store,
        wikipedia,
        arxiv,
        web_fetcher,
        knowledge_config,
        model_config: Arc::new(tokio::sync::RwLock::new(config.model.clone())),
    });

    // Start the scheduler service
    if config.scheduler.enabled {
        if let Err(e) = scheduler_service.start().await {
            tracing::error!("Failed to start scheduler: {}", e);
        }
    }

    tracing::info!("External knowledge base initialized (Wikipedia, ArXiv, Web fetcher)");

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
