//! Session management API endpoints
//!
//! Provides endpoints for persistent session management

use crate::models::{Session, SessionSummary};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;

/// Response for session review
#[derive(Debug, Serialize)]
pub struct SessionReviewResponse {
    pub session_id: String,
    pub reviewed: bool,
    pub quality_score: f32,
    pub training_examples_generated: usize,
    pub facts_extracted: usize,
    pub concepts_extracted: usize,
}

/// Response containing session data
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub id: String,
    pub user_id: Option<String>,
    pub messages: Vec<crate::models::Message>,
    pub memories: Vec<String>,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub status: String,
}

impl From<Session> for SessionResponse {
    fn from(session: Session) -> Self {
        Self {
            id: session.id.clone(),
            user_id: session.user_id.clone(),
            messages: session.messages,
            memories: session.memories,
            created_at: session.created_at.to_rfc3339(),
            ended_at: session.ended_at.map(|dt| dt.to_rfc3339()),
            status: match session.status {
                crate::models::SessionStatus::Active => "active".to_string(),
                crate::models::SessionStatus::Ended => "ended".to_string(),
                crate::models::SessionStatus::Error => "error".to_string(),
            },
        }
    }
}

/// List all sessions
///
/// GET /sessions
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<SessionSummary>> {
    if let Some(ref store) = state.session_store {
        match store.list_sessions() {
            Ok(sessions) => Json(sessions),
            Err(e) => {
                tracing::error!("Failed to list sessions: {}", e);
                Json(Vec::new())
            }
        }
    } else {
        // Fall back to in-memory session manager
        let agent_guard = state.agent.read().await;
        let sessions: Vec<SessionSummary> = match agent_guard.as_ref() {
            Some(agent) => agent
                .session_manager()
                .list_sessions()
                .into_iter()
                .map(|s| SessionSummary::from_session(&s))
                .collect(),
            None => Vec::new(),
        };
        Json(sessions)
    }
}

/// Get a specific session
///
/// GET /sessions/:id
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Option<SessionResponse>> {
    if let Some(ref store) = state.session_store {
        match store.load_session(&id) {
            Ok(Some(session)) => Json(Some(SessionResponse::from(session))),
            Ok(None) => Json(None),
            Err(e) => {
                tracing::error!("Failed to load session {}: {}", id, e);
                Json(None)
            }
        }
    } else {
        // Fall back to in-memory session manager
        let agent_guard = state.agent.read().await;
        let session = match agent_guard.as_ref() {
            Some(agent) => agent.session_manager().get_session(&id),
            None => None,
        };
        Json(session.map(SessionResponse::from))
    }
}

/// Delete a session
///
/// DELETE /sessions/:id
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let deleted = if let Some(ref store) = state.session_store {
        match store.delete_session(&id) {
            Ok(deleted) => deleted,
            Err(e) => {
                tracing::error!("Failed to delete session {}: {}", id, e);
                false
            }
        }
    } else {
        let agent_guard = state.agent.read().await;
        match agent_guard.as_ref() {
            Some(agent) => agent.session_manager().delete_session(&id),
            None => false,
        }
    };

    Json(serde_json::json!({
        "success": deleted,
        "id": id
    }))
}

/// End a session (mark as ended) and trigger session review
///
/// POST /sessions/:id/end
pub async fn end_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    // First, collect session messages for review before ending
    let (ended, review_response) = if let Some(ref store) = state.session_store {
        match store.load_session(&id) {
            Ok(Some(mut session)) => {
                let messages = session.messages.clone();

                // End the session
                session.end();
                let save_result = store.save_session(&session);

                // Perform session review (async for LLM enhancement)
                let review_result = if !messages.is_empty() {
                    state
                        .session_review_service
                        .review_session(&id, &messages)
                        .await
                } else {
                    crate::services::session_review::SessionReviewResult {
                        session_id: id.clone(),
                        quality_score: 0.0,
                        facts_extracted: 0,
                        concepts_extracted: 0,
                        training_examples_generated: 0,
                        memories_moved_to_training: 0,
                        memories_deleted: 0,
                        topics_for_research: Vec::new(),
                    }
                };

                // Generate and store training examples (LLM-enhanced when available)
                let examples = state
                    .session_review_service
                    .generate_training_examples(&messages)
                    .await;
                for example in examples {
                    state.batch_training_service.add_example(example).await;
                }

                tracing::info!(
                    "Session {} review: quality={:.2}, examples={}, facts={}, concepts={}",
                    id,
                    review_result.quality_score,
                    review_result.training_examples_generated,
                    review_result.facts_extracted,
                    review_result.concepts_extracted
                );

                let review_response = SessionReviewResponse {
                    session_id: id.clone(),
                    reviewed: true,
                    quality_score: review_result.quality_score,
                    training_examples_generated: review_result.training_examples_generated,
                    facts_extracted: review_result.facts_extracted,
                    concepts_extracted: review_result.concepts_extracted,
                };

                match save_result {
                    Ok(_) => (true, review_response),
                    Err(e) => {
                        tracing::error!("Failed to save ended session {}: {}", id, e);
                        (false, review_response)
                    }
                }
            }
            Ok(None) => (
                false,
                SessionReviewResponse {
                    session_id: id.clone(),
                    reviewed: false,
                    quality_score: 0.0,
                    training_examples_generated: 0,
                    facts_extracted: 0,
                    concepts_extracted: 0,
                },
            ),
            Err(e) => {
                tracing::error!("Failed to load session {}: {}", id, e);
                (
                    false,
                    SessionReviewResponse {
                        session_id: id.clone(),
                        reviewed: false,
                        quality_score: 0.0,
                        training_examples_generated: 0,
                        facts_extracted: 0,
                        concepts_extracted: 0,
                    },
                )
            }
        }
    } else {
        // Use in-memory manager
        let agent_guard = state.agent.read().await;
        match agent_guard.as_ref() {
            Some(agent) => {
                if let Some(mut session) = agent.session_manager().get_session(&id) {
                    let messages = session.messages.clone();
                    session.end();

                    // Perform session review (async for LLM enhancement)
                    let review_result = if !messages.is_empty() {
                        state
                            .session_review_service
                            .review_session(&id, &messages)
                            .await
                    } else {
                        crate::services::session_review::SessionReviewResult {
                            session_id: id.clone(),
                            quality_score: 0.0,
                            facts_extracted: 0,
                            concepts_extracted: 0,
                            training_examples_generated: 0,
                            memories_moved_to_training: 0,
                            memories_deleted: 0,
                            topics_for_research: Vec::new(),
                        }
                    };

                    // Generate and store training examples (LLM-enhanced when available)
                    let examples = state
                        .session_review_service
                        .generate_training_examples(&messages)
                        .await;
                    for example in examples {
                        state.batch_training_service.add_example(example).await;
                    }

                    let review_response = SessionReviewResponse {
                        session_id: id.clone(),
                        reviewed: true,
                        quality_score: review_result.quality_score,
                        training_examples_generated: review_result.training_examples_generated,
                        facts_extracted: review_result.facts_extracted,
                        concepts_extracted: review_result.concepts_extracted,
                    };

                    (true, review_response)
                } else {
                    (
                        false,
                        SessionReviewResponse {
                            session_id: id.clone(),
                            reviewed: false,
                            quality_score: 0.0,
                            training_examples_generated: 0,
                            facts_extracted: 0,
                            concepts_extracted: 0,
                        },
                    )
                }
            }
            None => (
                false,
                SessionReviewResponse {
                    session_id: id.clone(),
                    reviewed: false,
                    quality_score: 0.0,
                    training_examples_generated: 0,
                    facts_extracted: 0,
                    concepts_extracted: 0,
                },
            ),
        }
    };

    Json(serde_json::json!({
        "success": ended,
        "id": id,
        "review": review_response
    }))
}

/// Request to create a new session
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub user_id: Option<String>,
}

/// Response for session creation
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub id: String,
    pub created_at: String,
}

/// Create a new session
///
/// POST /sessions
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<CreateSessionResponse> {
    let session = if let Some(ref store) = state.session_store {
        let mut session = Session::new();
        session.user_id = req.user_id;
        match store.save_session(&session) {
            Ok(_) => Some(session),
            Err(e) => {
                tracing::error!("Failed to create session: {}", e);
                None
            }
        }
    } else {
        // Use in-memory manager
        let agent_guard = state.agent.read().await;
        match agent_guard.as_ref() {
            Some(agent) => {
                let session_arc = agent.session_manager().get_or_create_session();
                {
                    let mut s = session_arc.write();
                    s.user_id = req.user_id;
                }
                Some(session_arc.read().clone())
            }
            None => None,
        }
    };

    if let Some(session) = session {
        Json(CreateSessionResponse {
            id: session.id,
            created_at: session.created_at.to_rfc3339(),
        })
    } else {
        Json(CreateSessionResponse {
            id: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}
