//! Session management
//! 
//! Implements session management as specified in SPEC.md
//! 
//! This module re-exports Session from models and adds session management functionality.

pub use crate::models::Session;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// Session manager for handling multiple sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    current_session_id: Arc<RwLock<Option<String>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            current_session_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Get or create a session
    pub fn get_or_create_session(&self) -> Arc<RwLock<Session>> {
        let mut current_id = self.current_session_id.write();
        
        if let Some(id) = current_id.as_ref() {
            if let Some(session) = self.sessions.read().get(id) {
                return Arc::new(RwLock::new(session.clone()));
            }
        }

        // Create new session
        let session = Session::new();
        let id = session.id.clone();
        
        self.sessions.write().insert(id.clone(), session.clone());
        *current_id = Some(id.clone());
        
        Arc::new(RwLock::new(session))
    }

    /// Get a session by ID
    pub fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.read().get(id).cloned()
    }

    /// End current session
    pub fn end_current_session(&self) -> Option<Session> {
        let mut current_id = self.current_session_id.write();
        
        if let Some(id) = current_id.take() {
            if let Some(mut session) = self.sessions.write().remove(&id) {
                session.end();
                let closed = session.clone();
                self.sessions.write().insert(id, session);
                return Some(closed);
            }
        }
        
        None
    }

    /// Get current session (read-only reference)
    pub fn current_session(&self) -> Option<Session> {
        let current_id = self.current_session_id.read();
        current_id.as_ref().and_then(|id| {
            self.sessions.read().get(id).cloned()
        })
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions.read().values().cloned().collect()
    }

    /// Get active sessions count
    #[allow(dead_code)]
    pub fn active_sessions_count(&self) -> usize {
        self.sessions.read().values()
            .filter(|s| s.status == crate::models::SessionStatus::Active)
            .count()
    }

    /// Delete a session
    pub fn delete_session(&self, id: &str) -> bool {
        let mut sessions = self.sessions.write();
        
        // Don't allow deleting current session
        let current_id = self.current_session_id.read();
        if current_id.as_deref() == Some(id) {
            return false;
        }
        
        sessions.remove(id).is_some()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Role;

    #[test]
    fn test_session_manager() {
        let manager = SessionManager::new();
        
        // Get or create first session
        let session1 = manager.get_or_create_session();
        let id1 = session1.read().id.clone();
        
        // Get same session
        let session2 = manager.get_or_create_session();
        let id2 = session2.read().id.clone();
        
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_session_manager_multiple() {
        let manager = SessionManager::new();
        
        // Create first session
        let session1 = manager.get_or_create_session();
        let id1 = session1.read().id.clone();
        
        // Create second session (should be different since first was created)
        let session2 = manager.get_or_create_session();
        let id2 = session2.read().id.clone();
        
        // Should be same session
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_list_sessions() {
        let manager = SessionManager::new();
        
        // Create session
        let _session = manager.get_or_create_session();
        
        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_delete_session() {
        let manager = SessionManager::new();
        
        // Create session directly in manager for testing
        let session = manager.get_or_create_session();
        let id = session.read().id.clone();
        
        // Can't delete current session
        assert!(!manager.delete_session(&id));
        
        // Get returns the session
        assert!(manager.get_session(&id).is_some());
    }
}
