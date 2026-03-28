//! Persistent session storage using SQLite
//! 
//! Implements session persistence as specified in SPEC.md Phase 2

use crate::models::{Message, Session, SessionStatus, SessionSummary};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;

/// Message stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMessage {
    id: String,
    role: String,
    content: String,
    tool_calls: Option<String>,
    tool_results: Option<String>,
    memory_refs: Option<String>,
    reasoning: Option<String>,
}

impl From<&Message> for StoredMessage {
    fn from(msg: &Message) -> Self {
        Self {
            id: msg.id.clone(),
            role: match msg.role {
                crate::models::Role::System => "system".to_string(),
                crate::models::Role::User => "user".to_string(),
                crate::models::Role::Assistant => "assistant".to_string(),
            },
            content: msg.content.clone(),
            tool_calls: msg.tool_calls.as_ref().map(|tc| serde_json::to_string(tc).ok()).flatten(),
            tool_results: msg.tool_results.as_ref().map(|tr| serde_json::to_string(tr).ok()).flatten(),
            memory_refs: if msg.memory_refs.is_empty() { None } else { Some(msg.memory_refs.join(",")) },
            reasoning: msg.reasoning.clone(),
        }
    }
}

impl TryFrom<&StoredMessage> for Message {
    type Error = String;

    fn try_from(stored: &StoredMessage) -> Result<Self, Self::Error> {
        use crate::models::Role;
        
        let role = match stored.role.as_str() {
            "system" => Role::System,
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => return Err(format!("Unknown role: {}", stored.role)),
        };

        let tool_calls = stored.tool_calls.as_ref()
            .and_then(|tc| serde_json::from_str(tc).ok());
        let tool_results = stored.tool_results.as_ref()
            .and_then(|tr| serde_json::from_str(tr).ok());
        let memory_refs = stored.memory_refs.as_ref()
            .map(|refs| refs.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Ok(Message {
            id: stored.id.clone(),
            role,
            content: stored.content.clone(),
            content_parts: Vec::new(),
            tool_calls,
            tool_results,
            memory_refs,
            reasoning: stored.reasoning.clone(),
        })
    }
}

/// Persistent session store
#[allow(dead_code)]
pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SessionStore {
    /// Create a new session store
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)
            .context("Failed to open session store database")?;
        
        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                memories TEXT,
                created_at TEXT NOT NULL,
                ended_at TEXT,
                status TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_results TEXT,
                memory_refs TEXT,
                reasoning TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_session ON session_messages(session_id)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory store (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to create in-memory session store")?;
        
        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                memories TEXT,
                created_at TEXT NOT NULL,
                ended_at TEXT,
                status TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_results TEXT,
                memory_refs TEXT,
                reasoning TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Save a session
    pub fn save_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock();
        
        let memories = if session.memories.is_empty() {
            String::new()
        } else {
            session.memories.join(",")
        };

        let ended_at = session.ended_at
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        let status = match session.status {
            SessionStatus::Active => "active",
            SessionStatus::Ended => "ended",
            SessionStatus::Error => "error",
        };

        conn.execute(
            "INSERT OR REPLACE INTO sessions (id, user_id, memories, created_at, ended_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.id,
                session.user_id,
                memories,
                session.created_at.to_rfc3339(),
                ended_at,
                status,
            ],
        )?;

        // Clear existing messages for this session
        conn.execute(
            "DELETE FROM session_messages WHERE session_id = ?1",
            params![session.id],
        )?;

        // Save all messages
        for msg in &session.messages {
            let stored = StoredMessage::from(msg);
            conn.execute(
                "INSERT INTO session_messages (id, session_id, role, content, tool_calls, tool_results, memory_refs, reasoning, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    stored.id,
                    session.id,
                    stored.role,
                    stored.content,
                    stored.tool_calls,
                    stored.tool_results,
                    stored.memory_refs,
                    stored.reasoning,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
        }

        Ok(())
    }

    /// Load a session by ID
    pub fn load_session(&self, id: &str) -> Result<Option<Session>> {
        let conn = self.conn.lock();
        
        let mut stmt = conn.prepare(
            "SELECT id, user_id, memories, created_at, ended_at, status FROM sessions WHERE id = ?1"
        )?;

        let session = stmt.query_row(params![id], |row| {
            let memories_str: String = row.get(2)?;
            let memories = if memories_str.is_empty() {
                Vec::new()
            } else {
                memories_str.split(',').map(|s| s.to_string()).collect()
            };

            let created_at_str: String = row.get(3)?;
            let ended_at_str: Option<String> = row.get(4)?;
            
            let status_str: String = row.get(5)?;
            let status = match status_str.as_str() {
                "ended" => SessionStatus::Ended,
                "error" => SessionStatus::Error,
                _ => SessionStatus::Active,
            };

            Ok(Session {
                id: row.get(0)?,
                user_id: row.get(1)?,
                memories,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                ended_at: ended_at_str.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                }),
                status,
                messages: Vec::new(), // Will be loaded separately
            })
        });

        match session {
            Ok(mut session) => {
                // Load messages
                let mut msg_stmt = conn.prepare(
                    "SELECT id, role, content, tool_calls, tool_results, memory_refs, reasoning 
                     FROM session_messages WHERE session_id = ?1 ORDER BY created_at"
                )?;

                let messages: Vec<Message> = msg_stmt
                    .query_map(params![id], |row| {
                        let stored = StoredMessage {
                            id: row.get(0)?,
                            role: row.get(1)?,
                            content: row.get(2)?,
                            tool_calls: row.get(3)?,
                            tool_results: row.get(4)?,
                            memory_refs: row.get(5)?,
                            reasoning: row.get(6)?,
                        };
                        Ok(stored)
                    })?
                    .filter_map(|r| r.ok())
                    .filter_map(|stored| Message::try_from(&stored).ok())
                    .collect();

                session.messages = messages;
                Ok(Some(session))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all sessions (without messages)
    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let conn = self.conn.lock();
        
        let mut stmt = conn.prepare(
            "SELECT id, user_id, created_at, ended_at, status FROM sessions ORDER BY created_at DESC"
        )?;

        let sessions = stmt.query_map([], |row| {
            let ended_at_str: Option<String> = row.get(3)?;
            let status_str: String = row.get(4)?;
            
            Ok(SessionSummary {
                id: row.get(0)?,
                user_id: row.get(1)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                ended_at: ended_at_str.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                }),
                status: match status_str.as_str() {
                    "ended" => SessionStatus::Ended,
                    "error" => SessionStatus::Error,
                    _ => SessionStatus::Active,
                },
                message_count: 0, // Will be set separately
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(sessions)
    }

    /// Delete a session
    pub fn delete_session(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        
        // Delete messages first
        conn.execute(
            "DELETE FROM session_messages WHERE session_id = ?1",
            params![id],
        )?;

        // Delete session
        let deleted = conn.execute(
            "DELETE FROM sessions WHERE id = ?1",
            params![id],
        )?;

        Ok(deleted > 0)
    }

    /// Get session count
    pub fn session_count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Clean up old sessions (keep only last N)
    pub fn cleanup_old_sessions(&self, keep_last: usize) -> Result<usize> {
        let conn = self.conn.lock();
        
        let deleted = conn.execute(
            "DELETE FROM sessions WHERE id NOT IN (
                SELECT id FROM sessions ORDER BY created_at DESC LIMIT ?1
            )",
            params![keep_last as i64],
        )?;

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_store() {
        let store = SessionStore::in_memory().unwrap();
        
        // Create and save a session
        let mut session = Session::new();
        session.add_message(Message::user("Hello".to_string()));
        session.add_message(Message::assistant("Hi there!".to_string()));
        
        store.save_session(&session).unwrap();
        
        // Load it back
        let loaded = store.load_session(&session.id).unwrap().unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "Hello");
        assert_eq!(loaded.messages[1].content, "Hi there!");
    }

    #[test]
    fn test_list_sessions() {
        let store = SessionStore::in_memory().unwrap();
        
        // Create multiple sessions
        for i in 0..3 {
            let mut session = Session::new();
            session.add_message(Message::user(format!("Message {}", i)));
            store.save_session(&session).unwrap();
        }
        
        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn test_delete_session() {
        let store = SessionStore::in_memory().unwrap();
        
        let session = Session::new();
        store.save_session(&session).unwrap();
        
        assert!(store.delete_session(&session.id).unwrap());
        assert!(store.load_session(&session.id).unwrap().is_none());
    }

    #[test]
    fn test_session_with_user_id() {
        let store = SessionStore::in_memory().unwrap();
        
        let session = Session::with_user("user-123".to_string());
        store.save_session(&session).unwrap();
        
        let loaded = store.load_session(&session.id).unwrap().unwrap();
        assert_eq!(loaded.user_id, Some("user-123".to_string()));
    }
}
