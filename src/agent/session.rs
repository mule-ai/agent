//! Session management
//! 
//! Implements session management as specified in SPEC.md

use crate::models::{Message, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use uuid::Uuid;

/// Session as defined in SPEC
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub user_id: Option<String>,
    pub messages: Vec<Message>,
    pub memories: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: SessionStatus,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: None,
            messages: Vec::new(),
            memories: Vec::new(),
            created_at: chrono::Utc::now(),
            ended_at: None,
            status: SessionStatus::Active,
        }
    }

    pub fn with_user(user_id: String) -> Self {
        Self {
            user_id: Some(user_id),
            ..Self::new()
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn add_memory(&mut self, memory_id: String) {
        self.memories.push(memory_id);
    }

    pub fn end(&mut self) {
        self.status = SessionStatus::Ended;
        self.ended_at = Some(chrono::Utc::now());
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub fn active_sessions_count(&self) -> usize {
        self.sessions.read().values()
            .filter(|s| s.status == SessionStatus::Active)
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
    fn test_session_creation() {
        let session = Session::new();
        
        assert!(!session.id.is_empty());
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.messages.is_empty());
        assert!(session.memories.is_empty());
        assert!(session.ended_at.is_none());
    }

    #[test]
    fn test_session_with_user() {
        let session = Session::with_user("user-123".to_string());
        
        assert_eq!(session.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_session_add_message() {
        let mut session = Session::new();
        
        session.add_message(Message::user("Hello".to_string()));
        session.add_message(Message::assistant("Hi there!".to_string()));
        
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[1].role, Role::Assistant);
    }

    #[test]
    fn test_session_add_memory() {
        let mut session = Session::new();
        
        session.add_memory("mem-1".to_string());
        session.add_memory("mem-2".to_string());
        
        assert_eq!(session.memories.len(), 2);
    }

    #[test]
    fn test_session_end() {
        let mut session = Session::new();
        
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.ended_at.is_none());
        
        session.end();
        
        assert_eq!(session.status, SessionStatus::Ended);
        assert!(session.ended_at.is_some());
    }

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
