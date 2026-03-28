//! Session management API endpoints
//! 
//! Provides endpoints for persistent session management

use crate::models::{Session, SessionSummary};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;

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
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<SessionSummary>> {
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

/// End a session (mark as ended)
/// 
/// POST /sessions/:id/end
pub async fn end_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let ended = if let Some(ref store) = state.session_store {
        match store.load_session(&id) {
            Ok(Some(mut session)) => {
                session.end();
                match store.save_session(&session) {
                    Ok(_) => true,
                    Err(e) => {
                        tracing::error!("Failed to save ended session {}: {}", id, e);
                        false
                    }
                }
            }
            Ok(None) => false,
            Err(e) => {
                tracing::error!("Failed to load session {}: {}", id, e);
                false
            }
        }
    } else {
        // Use in-memory manager
        let agent_guard = state.agent.read().await;
        match agent_guard.as_ref() {
            Some(agent) => {
                if let Some(mut session) = agent.session_manager().get_session(&id) {
                    session.end();
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    };

    Json(serde_json::json!({
        "success": ended,
        "id": id
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
