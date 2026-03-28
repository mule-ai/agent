//! Service API endpoints

use crate::api::chat::AppState;
use crate::models::{Message, TrainingExample};
use crate::services::curiosity::{CuriosityStats, ExplorationResult, KnowledgeGap};
use crate::services::online_learning::{BufferStats, LearningStats, LearningUpdate};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Service status response
#[derive(Debug, Clone, Serialize)]
pub struct ServicesStatusResponse {
    pub session_review: ServiceInfo,
    pub memory_eviction: ServiceInfo,
    pub search_learning: ServiceInfo,
    pub curiosity: ServiceInfo,
}

/// Individual service info
#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    pub enabled: bool,
    pub name: String,
    pub description: String,
}

/// Get all services status
pub async fn services_status(
    State(_state): State<Arc<AppState>>,
) -> Json<ServicesStatusResponse> {
    Json(ServicesStatusResponse {
        session_review: ServiceInfo {
            enabled: true,
            name: "Session Review".to_string(),
            description: "Analyzes completed sessions to generate training data".to_string(),
        },
        memory_eviction: ServiceInfo {
            enabled: true,
            name: "Memory Eviction".to_string(),
            description: "Manages memory lifecycle and eviction policies".to_string(),
        },
        search_learning: ServiceInfo {
            enabled: true,
            name: "Search Learning".to_string(),
            description: "Researches topics to fill knowledge gaps".to_string(),
        },
        curiosity: ServiceInfo {
            enabled: true,
            name: "Curiosity Engine".to_string(),
            description: "Autonomously explores topics the agent doesn't understand well".to_string(),
        },
    })
}

/// Request to run session review
#[derive(Debug, Deserialize)]
pub struct SessionReviewRequest {
    pub session_id: Option<String>,
}

/// Session review result
#[derive(Debug, Clone, Serialize)]
pub struct SessionReviewResult {
    pub success: bool,
    pub message: String,
    pub quality_score: Option<f32>,
    pub facts_extracted: usize,
    pub concepts_extracted: usize,
    pub training_examples: usize,
}

/// Run session review on current or specified session
pub async fn run_session_review(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SessionReviewRequest>,
) -> Json<SessionReviewResult> {
    let service = &state.session_review_service;
    
    // Get session to review
    let session = if let Some(id) = &req.session_id {
        state.agent.read().await
            .as_ref()
            .and_then(|a| a.current_session())
            .filter(|s| s.id == *id)
    } else {
        state.agent.read().await
            .as_ref()
            .and_then(|a| a.current_session())
    };

    match session {
        Some(session) => {
            let messages: Vec<Message> = session.messages.clone();
            
            // Analyze session
            let analysis = service.analyze_session(&messages);
            let training_examples = service.generate_training_examples(&messages);
            let memories = service.generate_memories(&messages);
            
            // Store generated memories
            let memory_store = state.agent.read().await
                .as_ref()
                .map(|a| a.memory_store());
            
            if let Some(store) = memory_store {
                for memory in &memories {
                    if let Err(e) = store.store(memory) {
                        tracing::warn!("Failed to store memory: {}", e);
                    }
                }
            }
            
            Json(SessionReviewResult {
                success: true,
                message: format!("Reviewed session {}", session.id),
                quality_score: Some(analysis.quality_score),
                facts_extracted: analysis.facts.len(),
                concepts_extracted: analysis.concepts.len(),
                training_examples: training_examples.len(),
            })
        }
        None => Json(SessionReviewResult {
            success: false,
            message: "No session found to review".to_string(),
            quality_score: None,
            facts_extracted: 0,
            concepts_extracted: 0,
            training_examples: 0,
        }),
    }
}

/// Request to run memory eviction
#[derive(Debug, Deserialize)]
pub struct EvictionRequest {
    pub namespace: Option<String>,
}

/// Eviction result
#[derive(Debug, Clone, Serialize)]
pub struct EvictionResult {
    pub success: bool,
    pub message: String,
    pub processed: usize,
    pub kept: usize,
    pub moved_to_training: usize,
    pub deleted: usize,
}

/// Run memory eviction
pub async fn run_eviction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EvictionRequest>,
) -> Json<EvictionResult> {
    let service = &state.memory_eviction_service;
    let namespace = req.namespace.unwrap_or_else(|| "retrieval".to_string());
    
    // Get memories to process
    let memories = state.memory_store.list(&namespace, 1000)
        .unwrap_or_default();
    
    if memories.is_empty() {
        return Json(EvictionResult {
            success: true,
            message: "No memories to process".to_string(),
            processed: 0,
            kept: 0,
            moved_to_training: 0,
            deleted: 0,
        });
    }
    
    // Process memories
    let mut results = Vec::new();
    for memory in memories {
        let result = service.process_memory(&memory);
        results.push(result);
    }
    
    // Count results
    let kept = results.iter().filter(|r| r.action == "kept").count();
    let moved = results.iter().filter(|r| r.action == "moved_to_training").count();
    let deleted = results.iter().filter(|r| r.action == "deleted").count();
    
    // Apply deletions
    for result in &results {
        if result.action == "deleted" {
            if let Err(e) = state.memory_store.delete(&result.memory_id) {
                tracing::warn!("Failed to delete memory {}: {}", result.memory_id, e);
            }
        }
    }
    
    Json(EvictionResult {
        success: true,
        message: format!("Processed {} memories", results.len()),
        processed: results.len(),
        kept,
        moved_to_training: moved,
        deleted,
    })
}

/// Request to run search learning
#[derive(Debug, Deserialize)]
pub struct SearchLearningRequest {
    pub topic: Option<String>,
    pub topics: Option<Vec<String>>,
}

/// Search learning result
#[derive(Debug, Clone, Serialize)]
pub struct SearchLearningResult {
    pub success: bool,
    pub message: String,
    pub topics_researched: usize,
    pub concepts_learned: usize,
}

/// Run search learning
pub async fn run_search_learning(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchLearningRequest>,
) -> Json<SearchLearningResult> {
    let service = &state.search_learning_service;
    
    // Add topics to research
    if let Some(topic) = req.topic {
        service.add_knowledge_gap(&topic).await;
    }
    
    if let Some(topics) = req.topics {
        for topic in topics {
            service.add_knowledge_gap(&topic).await;
        }
    }
    
    // Process pending topics
    let processed = service.process_pending().await;
    
    // Store learned concepts
    for topic in &processed {
        let concepts = service.learn_from_topic(topic).await;
        let _ = concepts.len(); // Track count without storing
        
        for memory in concepts {
            if let Err(e) = state.memory_store.store(&memory) {
                tracing::warn!("Failed to store learned concept: {}", e);
            }
        }
    }
    
    let stats = service.get_stats().await;
    
    Json(SearchLearningResult {
        success: true,
        message: format!("Researched {} topics", processed.len()),
        topics_researched: stats.topics_researched,
        concepts_learned: stats.concepts_learned,
    })
}

// ============================================
// Curiosity Engine API Endpoints (Phase 3)
// ============================================

/// Get curiosity engine statistics
pub async fn curiosity_stats(
    State(state): State<Arc<AppState>>,
) -> Json<CuriosityStats> {
    let engine = &state.curiosity_engine;
    let stats = engine.get_stats().await;
    Json(stats)
}

/// Detect knowledge gaps from a conversation
#[derive(Debug, Deserialize)]
pub struct DetectGapsRequest {
    pub messages: Vec<Message>,
}

/// Detect gaps response
#[derive(Debug, Clone, Serialize)]
pub struct DetectGapsResponse {
    pub success: bool,
    pub gaps_detected: usize,
    pub gaps: Vec<KnowledgeGap>,
}

/// Detect knowledge gaps in a conversation
pub async fn detect_gaps(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DetectGapsRequest>,
) -> Json<DetectGapsResponse> {
    let engine = &state.curiosity_engine;
    let gaps = engine.detect_gaps(&req.messages).await;
    
    Json(DetectGapsResponse {
        success: true,
        gaps_detected: gaps.len(),
        gaps,
    })
}

/// Get all detected knowledge gaps
pub async fn list_gaps(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<KnowledgeGap>> {
    let engine = &state.curiosity_engine;
    let gaps = engine.get_all_gaps().await;
    Json(gaps)
}

/// Get pending gaps that need exploration
pub async fn pending_gaps(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<KnowledgeGap>> {
    let engine = &state.curiosity_engine;
    let gaps = engine.get_pending_gaps().await;
    Json(gaps)
}

/// Explore a specific knowledge gap
#[derive(Debug, Deserialize)]
pub struct ExploreGapRequest {
    pub gap_id: String,
}

/// Exploration response
#[derive(Debug, Clone, Serialize)]
pub struct ExploreGapResponse {
    pub success: bool,
    pub message: String,
    pub result: Option<ExplorationResult>,
}

/// Explore a knowledge gap
pub async fn explore_gap(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExploreGapRequest>,
) -> Json<ExploreGapResponse> {
    let engine = &state.curiosity_engine;
    
    match engine.explore_gap(&req.gap_id).await {
        Ok(result) => {
            // Convert result to memories and store
            let memories = engine.result_to_memories(&result, "training").await;
            for memory in memories {
                if let Err(e) = state.memory_store.store(&memory) {
                    tracing::warn!("Failed to store curiosity memory: {}", e);
                }
            }
            
            Json(ExploreGapResponse {
                success: true,
                message: format!("Explored gap {}", req.gap_id),
                result: Some(result),
            })
        }
        Err(e) => Json(ExploreGapResponse {
            success: false,
            message: format!("Failed to explore gap: {}", e),
            result: None,
        }),
    }
}

/// Process the curiosity exploration queue
#[derive(Debug, Deserialize)]
pub struct ProcessQueueRequest {
    #[allow(dead_code)]
    pub max_explorations: Option<usize>,
}

/// Process queue response
#[derive(Debug, Clone, Serialize)]
pub struct ProcessQueueResponse {
    pub success: bool,
    pub explorations_performed: usize,
    pub results: Vec<ExplorationResult>,
}

/// Process the exploration queue
pub async fn process_curiosity_queue(
    State(state): State<Arc<AppState>>,
    Json(_req): Json<ProcessQueueRequest>,
) -> Json<ProcessQueueResponse> {
    let engine = &state.curiosity_engine;
    
    let results = engine.process_queue().await;
    
    // Store all learned memories
    for result in &results {
        let memories = engine.result_to_memories(result, "training").await;
        for memory in memories {
            if let Err(e) = state.memory_store.store(&memory) {
                tracing::warn!("Failed to store curiosity memory: {}", e);
            }
        }
    }
    
    let stats = engine.get_stats().await;
    
    Json(ProcessQueueResponse {
        success: true,
        explorations_performed: stats.explorations_performed,
        results,
    })
}

/// Dismiss a knowledge gap
#[derive(Debug, Deserialize)]
pub struct DismissGapRequest {
    pub gap_id: String,
}

/// Dismiss gap response
#[derive(Debug, Clone, Serialize)]
pub struct DismissGapResponse {
    pub success: bool,
    pub message: String,
}

/// Dismiss a knowledge gap (mark as not worth exploring)
pub async fn dismiss_gap(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DismissGapRequest>,
) -> Json<DismissGapResponse> {
    let engine = &state.curiosity_engine;
    engine.dismiss_gap(&req.gap_id).await;
    
    Json(DismissGapResponse {
        success: true,
        message: format!("Dismissed gap {}", req.gap_id),
    })
}

// ============================================
// Online Learning API Endpoints (Phase 3 - Continuous RL)
// ============================================

/// Get online learning statistics
pub async fn learning_stats(
    State(state): State<Arc<AppState>>,
) -> Json<LearningStats> {
    let service = &state.online_learning_service;
    let stats = service.get_stats().await;
    Json(stats)
}

/// Get buffer statistics
pub async fn learning_buffer_stats(
    State(state): State<Arc<AppState>>,
) -> Json<BufferStats> {
    let service = &state.online_learning_service;
    let stats = service.get_buffer_stats().await;
    Json(stats)
}

/// Request to add training example
#[derive(Debug, Deserialize)]
pub struct AddExampleRequest {
    pub prompt: String,
    pub completion: String,
}

/// Add example response
#[derive(Debug, Clone, Serialize)]
pub struct AddExampleResponse {
    pub success: bool,
    pub message: String,
    pub example_id: Option<String>,
}

/// Add a training example to the replay buffer
pub async fn add_learning_example(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddExampleRequest>,
) -> Json<AddExampleResponse> {
    let service = &state.online_learning_service;
    
    let example = TrainingExample {
        id: uuid::Uuid::new_v4().to_string(),
        prompt: req.prompt,
        completion: req.completion,
        reasoning: String::new(),
        reward: 0.0,
        source: crate::models::TrainingSource::Manual,
        created_at: chrono::Utc::now(),
        quality_score: 0.5,
        used_in_training: false,
    };
    
    let example_id = example.id.clone();
    service.add_experience(example).await;
    
    Json(AddExampleResponse {
        success: true,
        message: "Example added to replay buffer".to_string(),
        example_id: Some(example_id),
    })
}

/// Run online learning update
#[derive(Debug, Deserialize)]
pub struct RunLearnRequest {
    #[allow(dead_code)]
    pub batch_size: Option<usize>,
}

/// Learning update response
#[derive(Debug, Clone, Serialize)]
pub struct RunLearnResponse {
    pub success: bool,
    pub message: String,
    pub update: Option<LearningUpdate>,
}

/// Perform an online learning update
pub async fn run_online_learn(
    State(state): State<Arc<AppState>>,
    Json(_req): Json<RunLearnRequest>,
) -> Json<RunLearnResponse> {
    let service = &state.online_learning_service;
    
    // Check if ready
    if !service.is_ready().await {
        let buffer_stats = service.get_buffer_stats().await;
        return Json(RunLearnResponse {
            success: false,
            message: format!("Not ready for training. Buffer has {} examples, need at least 50", buffer_stats.total),
            update: None,
        });
    }
    
    // Perform learning update
    match service.learn().await {
        Ok(update) => {
            Json(RunLearnResponse {
                success: true,
                message: format!("Processed {} examples", update.examples_processed),
                update: Some(update),
            })
        }
        Err(e) => Json(RunLearnResponse {
            success: false,
            message: format!("Learning failed: {}", e),
            update: None,
        }),
    }
}

/// Get learned concepts
#[derive(Debug, Clone, Serialize)]
pub struct ConceptsResponse {
    pub concepts: Vec<ConceptInfo>,
}

/// Concept information
#[derive(Debug, Clone, Serialize)]
pub struct ConceptInfo {
    pub concept: String,
    pub strength: f32,
}

/// Get all learned concepts
pub async fn learning_concepts(
    State(state): State<Arc<AppState>>,
) -> Json<ConceptsResponse> {
    let service = &state.online_learning_service;
    
    let concepts = service.get_concepts().await;
    
    let concept_infos: Vec<ConceptInfo> = concepts
        .into_iter()
        .map(|(concept, embedding)| {
            let strength = embedding.first().copied().unwrap_or(0.0);
            ConceptInfo { concept, strength }
        })
        .collect();
    
    Json(ConceptsResponse {
        concepts: concept_infos,
    })
}

/// Request to add session experiences
#[derive(Debug, Deserialize)]
pub struct AddSessionExperiencesRequest {
    pub messages: Vec<Message>,
}

/// Add session experiences response
#[derive(Debug, Clone, Serialize)]
pub struct AddSessionExperiencesResponse {
    pub success: bool,
    pub message: String,
    pub examples_added: usize,
}

/// Add all experiences from a session
pub async fn add_session_experiences(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddSessionExperiencesRequest>,
) -> Json<AddSessionExperiencesResponse> {
    let service = &state.online_learning_service;
    
    let initial_count = service.get_stats().await.examples_collected;
    service.add_session_experiences(&req.messages).await;
    let final_count = service.get_stats().await.examples_collected;
    
    Json(AddSessionExperiencesResponse {
        success: true,
        message: "Session experiences added".to_string(),
        examples_added: final_count - initial_count,
    })
}

/// Prune trained examples from buffer
#[derive(Debug, Clone, Serialize)]
pub struct PruneBufferResponse {
    pub success: bool,
    pub message: String,
    pub pruned: usize,
}

/// Prune trained examples from replay buffer
pub async fn prune_learning_buffer(
    State(state): State<Arc<AppState>>,
) -> Json<PruneBufferResponse> {
    let service = &state.online_learning_service;
    
    let pruned = service.prune_trained().await;
    
    Json(PruneBufferResponse {
        success: true,
        message: format!("Pruned {} trained examples", pruned),
        pruned,
    })
}

// ============================================
// Self-Improvement API Endpoints (Phase 3)
// ============================================

use crate::services::self_improve::{
    InteractionSummary, Improvement, ImprovementStatus,
    CodeAnalysisResult, CodeImprovement, CodePattern, SearchCodeResult,
    ImprovementHistoryEntry,
};
use crate::services::theory_of_mind::{
    MessageContext as ToMMessageContext, UserMentalState,
    ToMAnalysis, ToMStats,
};

/// Get self-improvement engine statistics
pub async fn self_improve_stats(
    State(state): State<Arc<AppState>>,
) -> Json<crate::services::SelfImproveStats> {
    let engine = &state.self_improve_engine;
    let stats = engine.get_stats().await;
    Json(stats)
}

/// Analyze and generate improvements
#[derive(Debug, Deserialize)]
pub struct AnalyzeImproveRequest {
    pub interactions: Vec<InteractionSummary>,
    pub tool_usage: std::collections::HashMap<String, usize>,
    pub errors: Vec<String>,
}

/// Analysis response
#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeImproveResponse {
    pub success: bool,
    pub improvements_generated: usize,
    pub improvements: Vec<Improvement>,
}

/// Run self-improvement analysis
pub async fn run_self_improve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnalyzeImproveRequest>,
) -> Json<AnalyzeImproveResponse> {
    let engine = &state.self_improve_engine;
    
    let improvements = engine.analyze_and_improve(
        &req.interactions,
        &req.tool_usage,
        &req.errors,
    ).await;
    
    Json(AnalyzeImproveResponse {
        success: true,
        improvements_generated: improvements.len(),
        improvements,
    })
}

/// Get all improvements
#[derive(Debug, Deserialize)]
pub struct GetImprovementsRequest {
    pub status: Option<String>,
}

/// Get improvements response
#[derive(Debug, Clone, Serialize)]
pub struct GetImprovementsResponse {
    pub improvements: Vec<Improvement>,
}

/// Get improvements (optionally filtered by status)
pub async fn get_improvements(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GetImprovementsRequest>,
) -> Json<GetImprovementsResponse> {
    let engine = &state.self_improve_engine;
    
    let status_filter = req.status.as_ref().and_then(|s| match s.as_str() {
        "pending" => Some(ImprovementStatus::Pending),
        "generated" => Some(ImprovementStatus::Generated),
        "applied" => Some(ImprovementStatus::Applied),
        "rejected" => Some(ImprovementStatus::Rejected),
        _ => None,
    });
    
    let improvements = engine.get_improvements(status_filter).await;
    
    Json(GetImprovementsResponse { improvements })
}

/// Apply an improvement
#[derive(Debug, Deserialize)]
pub struct ApplyImprovementRequest {
    pub improvement_id: String,
}

/// Apply improvement response
#[derive(Debug, Clone, Serialize)]
pub struct ApplyImprovementResponse {
    pub success: bool,
    pub message: String,
}

/// Apply a generated improvement
pub async fn apply_improvement(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyImprovementRequest>,
) -> Json<ApplyImprovementResponse> {
    let engine = &state.self_improve_engine;
    
    match engine.apply_improvement(&req.improvement_id).await {
        Ok(()) => Json(ApplyImprovementResponse {
            success: true,
            message: format!("Applied improvement {}", req.improvement_id),
        }),
        Err(e) => Json(ApplyImprovementResponse {
            success: false,
            message: e,
        }),
    }
}

/// Reject an improvement
#[derive(Debug, Deserialize)]
pub struct RejectImprovementRequest {
    pub improvement_id: String,
    pub reason: String,
}

/// Reject improvement response
#[derive(Debug, Clone, Serialize)]
pub struct RejectImprovementResponse {
    pub success: bool,
    pub message: String,
}

/// Reject a generated improvement
pub async fn reject_improvement(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RejectImprovementRequest>,
) -> Json<RejectImprovementResponse> {
    let engine = &state.self_improve_engine;
    
    match engine.reject_improvement(&req.improvement_id, &req.reason).await {
        Ok(()) => Json(RejectImprovementResponse {
            success: true,
            message: format!("Rejected improvement {}", req.improvement_id),
        }),
        Err(e) => Json(RejectImprovementResponse {
            success: false,
            message: e,
        }),
    }
}

/// Rollback an applied improvement
#[derive(Debug, Deserialize)]
pub struct RollbackImprovementRequest {
    pub improvement_id: String,
}

/// Rollback improvement response
#[derive(Debug, Clone, Serialize)]
pub struct RollbackImprovementResponse {
    pub success: bool,
    pub message: String,
}

/// Rollback an applied improvement
pub async fn rollback_improvement(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RollbackImprovementRequest>,
) -> Json<RollbackImprovementResponse> {
    let engine = &state.self_improve_engine;
    
    match engine.rollback_improvement(&req.improvement_id).await {
        Ok(()) => Json(RollbackImprovementResponse {
            success: true,
            message: format!("Rolled back improvement {}", req.improvement_id),
        }),
        Err(e) => Json(RollbackImprovementResponse {
            success: false,
            message: e,
        }),
    }
}

/// Get current system prompt
pub async fn get_system_prompt(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let engine = &state.self_improve_engine;
    let prompt = engine.get_current_prompt().await;
    Json(serde_json::json!({ "prompt": prompt }))
}

/// Update system prompt
#[derive(Debug, Deserialize)]
pub struct UpdatePromptRequest {
    pub prompt: String,
}

/// Update prompt response
#[derive(Debug, Clone, Serialize)]
pub struct UpdatePromptResponse {
    pub success: bool,
    pub message: String,
}

/// Update the system prompt
pub async fn update_system_prompt(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdatePromptRequest>,
) -> Json<UpdatePromptResponse> {
    let engine = &state.self_improve_engine;
    engine.update_prompt(req.prompt).await;
    
    Json(UpdatePromptResponse {
        success: true,
        message: "System prompt updated".to_string(),
    })
}

// ============================================
// Code Analysis API Endpoints (Self-Improvement)
// ============================================

/// Analyze code from search results
#[derive(Debug, Deserialize)]
pub struct AnalyzeCodeRequest {
    pub query: String,
    pub results: Vec<SearchCodeResult>,
}

/// Analyze code response
#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeCodeResponse {
    pub success: bool,
    pub patterns_detected: usize,
    pub improvements_found: usize,
    pub result: Option<CodeAnalysisResult>,
}

/// Analyze code from search results to detect patterns and suggest improvements
pub async fn analyze_code_from_search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnalyzeCodeRequest>,
) -> Json<AnalyzeCodeResponse> {
    let engine = &state.self_improve_engine;
    
    let result = engine.analyze_code_from_search(&req.query, &req.results).await;
    
    Json(AnalyzeCodeResponse {
        success: true,
        patterns_detected: result.patterns.len(),
        improvements_found: result.improvements.len(),
        result: Some(result),
    })
}

/// Get all detected code patterns
#[derive(Debug, Clone, Serialize)]
pub struct CodePatternsResponse {
    pub patterns: Vec<CodePattern>,
}

/// Get all detected code patterns
pub async fn get_code_patterns(
    State(state): State<Arc<AppState>>,
) -> Json<CodePatternsResponse> {
    let engine = &state.self_improve_engine;
    let patterns = engine.get_code_patterns().await;
    Json(CodePatternsResponse { patterns })
}

/// Get all code improvements
#[derive(Debug, Clone, Serialize)]
pub struct CodeImprovementsResponse {
    pub improvements: Vec<CodeImprovement>,
}

/// Get all detected code improvements
pub async fn get_code_improvements(
    State(state): State<Arc<AppState>>,
) -> Json<CodeImprovementsResponse> {
    let engine = &state.self_improve_engine;
    let improvements = engine.get_code_improvements().await;
    Json(CodeImprovementsResponse { improvements })
}

/// Apply a code improvement to actual agent code
#[derive(Debug, Deserialize)]
pub struct ApplyCodeImprovementRequest {
    pub improvement_id: String,
}

/// Apply code improvement response
#[derive(Debug, Clone, Serialize)]
pub struct ApplyCodeImprovementResponse {
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
}

/// Apply a code improvement to the agent codebase
pub async fn apply_code_improvement(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyCodeImprovementRequest>,
) -> Json<ApplyCodeImprovementResponse> {
    let engine = &state.self_improve_engine;
    
    match engine.apply_code_improvement(&req.improvement_id).await {
        Ok(path) => Json(ApplyCodeImprovementResponse {
            success: true,
            message: format!("Applied code improvement to {}", path),
            file_path: Some(path),
        }),
        Err(e) => Json(ApplyCodeImprovementResponse {
            success: false,
            message: e,
            file_path: None,
        }),
    }
}

/// Rollback a code improvement
#[derive(Debug, Deserialize)]
pub struct RollbackCodeImprovementRequest {
    pub improvement_id: String,
}

/// Rollback code improvement response
#[derive(Debug, Clone, Serialize)]
pub struct RollbackCodeImprovementResponse {
    pub success: bool,
    pub message: String,
}

/// Rollback an applied code improvement
pub async fn rollback_code_improvement(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RollbackCodeImprovementRequest>,
) -> Json<RollbackCodeImprovementResponse> {
    let engine = &state.self_improve_engine;
    
    match engine.rollback_code_improvement(&req.improvement_id).await {
        Ok(()) => Json(RollbackCodeImprovementResponse {
            success: true,
            message: format!("Rolled back improvement {}", req.improvement_id),
        }),
        Err(e) => Json(RollbackCodeImprovementResponse {
            success: false,
            message: e,
        }),
    }
}

/// Get improvement history
#[derive(Debug, Clone, Serialize)]
pub struct ImprovementHistoryResponse {
    pub history: Vec<ImprovementHistoryEntry>,
}

/// Get all improvement history entries
pub async fn get_improvement_history(
    State(state): State<Arc<AppState>>,
) -> Json<ImprovementHistoryResponse> {
    let engine = &state.self_improve_engine;
    let history = engine.get_improvement_history().await;
    Json(ImprovementHistoryResponse { history })
}

/// Get extended self-improvement statistics
pub async fn self_improve_extended_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let engine = &state.self_improve_engine;
    let stats = engine.get_extended_stats().await;
    Json(stats)
}

// ============================================
// Theory of Mind API Endpoints (Phase 3)
// ============================================

/// Get Theory of Mind statistics
pub async fn tom_stats(
    State(state): State<Arc<AppState>>,
) -> Json<ToMStats> {
    let engine = &state.theory_of_mind_engine;
    let stats = engine.get_stats().await;
    Json(stats)
}

/// Update user mental model
#[derive(Debug, Deserialize)]
pub struct UpdateUserModelRequest {
    pub user_id: String,
    pub messages: Vec<ToMMessageContext>,
}

/// Update user model response
#[derive(Debug, Clone, Serialize)]
pub struct UpdateUserModelResponse {
    pub success: bool,
    pub model: Option<UserMentalState>,
}

/// Update a user's mental model based on messages
pub async fn update_user_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateUserModelRequest>,
) -> Json<UpdateUserModelResponse> {
    let engine = &state.theory_of_mind_engine;
    
    match engine.update_user_model(&req.user_id, &req.messages).await {
        Ok(model) => Json(UpdateUserModelResponse {
            success: true,
            model: Some(model),
        }),
        Err(_) => Json(UpdateUserModelResponse {
            success: false,
            model: None,
        }),
    }
}

/// Get user mental model
#[derive(Debug, Clone, Serialize)]
pub struct GetUserModelResponse {
    pub model: Option<UserMentalState>,
}

/// Get a user's mental model
pub async fn get_user_model(
    State(state): State<Arc<AppState>>,
) -> Json<GetUserModelResponse> {
    // Get from query param user_id if provided, else use "default"
    // For now, return default user model
    let engine = &state.theory_of_mind_engine;
    let model = engine.get_user_model("default").await;
    
    Json(GetUserModelResponse { model })
}

/// Get all user models
pub async fn get_all_user_models(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<UserMentalState>> {
    let engine = &state.theory_of_mind_engine;
    let models = engine.get_all_user_models().await;
    Json(models)
}

/// Analyze user for response recommendations
#[derive(Debug, Deserialize)]
pub struct AnalyzeForResponseRequest {
    pub user_id: String,
}

/// Analyze user for response
pub async fn analyze_for_response(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnalyzeForResponseRequest>,
) -> Json<Option<ToMAnalysis>> {
    let engine = &state.theory_of_mind_engine;
    let analysis = engine.analyze_for_response(&req.user_id).await;
    Json(analysis)
}

/// Get conversation history for a user
#[derive(Debug, Clone, Serialize)]
pub struct GetConversationHistoryResponse {
    pub user_id: String,
    pub messages: Vec<ToMMessageContext>,
}

/// Get conversation history
pub async fn get_conversation_history(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<GetConversationHistoryResponse>> {
    let engine = &state.theory_of_mind_engine;
    
    // For now, return history for default user
    let messages = engine.get_conversation_history("default").await;
    
    Json(vec![GetConversationHistoryResponse {
        user_id: "default".to_string(),
        messages,
    }])
}

/// Clear user model
#[derive(Debug, Deserialize)]
pub struct ClearUserModelRequest {
    pub user_id: String,
}

/// Clear user model response
#[derive(Debug, Clone, Serialize)]
pub struct ClearUserModelResponse {
    pub success: bool,
    pub message: String,
}

/// Clear a user's mental model
pub async fn clear_user_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClearUserModelRequest>,
) -> Json<ClearUserModelResponse> {
    let engine = &state.theory_of_mind_engine;
    engine.clear_user_model(&req.user_id).await;
    
    Json(ClearUserModelResponse {
        success: true,
        message: format!("Cleared model for user {}", req.user_id),
    })
}

/// Update trust level
#[derive(Debug, Deserialize)]
pub struct UpdateTrustRequest {
    pub user_id: String,
    pub delta: f32,
}

/// Update trust response
#[derive(Debug, Clone, Serialize)]
pub struct UpdateTrustResponse {
    pub success: bool,
    pub message: String,
}

/// Update trust level for a user
pub async fn update_trust(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateTrustRequest>,
) -> Json<UpdateTrustResponse> {
    let engine = &state.theory_of_mind_engine;
    
    match engine.update_trust(&req.user_id, req.delta).await {
        Ok(()) => Json(UpdateTrustResponse {
            success: true,
            message: format!("Updated trust for user {}", req.user_id),
        }),
        Err(e) => Json(UpdateTrustResponse {
            success: false,
            message: e,
        }),
    }
}

/// Satisfy an intention
#[derive(Debug, Deserialize)]
pub struct SatisfyIntentionRequest {
    pub user_id: String,
    pub intention_id: String,
}

/// Satisfy intention response
#[derive(Debug, Clone, Serialize)]
pub struct SatisfyIntentionResponse {
    pub success: bool,
    pub message: String,
}

/// Mark an intention as satisfied
pub async fn satisfy_intention(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SatisfyIntentionRequest>,
) -> Json<SatisfyIntentionResponse> {
    let engine = &state.theory_of_mind_engine;
    
    match engine.satisfy_intention(&req.user_id, &req.intention_id).await {
        Ok(()) => Json(SatisfyIntentionResponse {
            success: true,
            message: format!("Satisfied intention {}", req.intention_id),
        }),
        Err(e) => Json(SatisfyIntentionResponse {
            success: false,
            message: e,
        }),
    }
}

// ============================================
// Scheduler API Endpoints (TASK 5: Scheduled Batch Training)
// ============================================

use crate::services::scheduler::{SchedulerStats, SchedulerConfig};

/// Scheduler statistics response
#[derive(Debug, Clone, Serialize)]
pub struct SchedulerStatusResponse {
    pub enabled: bool,
    pub config: SchedulerConfig,
    pub stats: SchedulerStats,
}

/// Get scheduler statistics
pub async fn scheduler_stats(
    State(state): State<Arc<AppState>>,
) -> Json<SchedulerStatusResponse> {
    let scheduler = &state.scheduler_service;
    let stats = scheduler.get_stats().await;
    let config = scheduler.config().clone();
    
    Json(SchedulerStatusResponse {
        enabled: config.enabled,
        config,
        stats,
    })
}

/// Request to trigger batch training manually
#[derive(Debug, Deserialize)]
pub struct TriggerTrainingRequest {
    #[allow(dead_code)]
    pub force: Option<bool>,
}

/// Trigger training response
#[derive(Debug, Clone, Serialize)]
pub struct TriggerTrainingResponse {
    pub success: bool,
    pub message: String,
    pub examples_count: usize,
}

/// Manually trigger batch training (bypass schedule)
pub async fn scheduler_trigger_training(
    State(state): State<Arc<AppState>>,
    Json(_req): Json<TriggerTrainingRequest>,
) -> Json<TriggerTrainingResponse> {
    let scheduler = &state.scheduler_service;
    
    match scheduler.trigger_batch_training().await {
        Ok(()) => {
            let examples = state.batch_training_service.example_count().await;
            Json(TriggerTrainingResponse {
                success: true,
                message: "Batch training triggered successfully".to_string(),
                examples_count: examples,
            })
        }
        Err(e) => {
            Json(TriggerTrainingResponse {
                success: false,
                message: e.to_string(),
                examples_count: 0,
            })
        }
    }
}
