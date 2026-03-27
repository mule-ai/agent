//! SQLite + Tantivy memory storage
//! 
//! Implements the memory store as specified in SPEC.md

use crate::models::{Memory, MemoryType, QueryResult};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, ReloadPolicy, TantivyDocument};

/// Memory store trait for abstraction
pub trait MemoryStore: Send + Sync {
    fn store(&self, memory: &Memory) -> Result<()>;
    fn update(&self, memory: &Memory) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<Memory>>;
    fn list(&self, namespace: &str, limit: usize) -> Result<Vec<Memory>>;
    fn query(&self, embedding: &[f32], namespace: &str, limit: usize, min_score: f32) -> Result<Vec<QueryResult>>;
}

/// SQLite + Tantivy based memory store
pub struct SqliteMemoryStore {
    conn: Arc<Mutex<Connection>>,
    index: Index,
    schema: Schema,
    // Schema fields
    id_field: Field,
    content_field: Field,
    namespace_field: Field,
    tags_field: Field,
    memory_type_field: Field,
    created_at_field: Field,
}

impl SqliteMemoryStore {
    /// Create a new memory store
    pub fn new(db_path: impl AsRef<Path>, index_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        let index_path = index_path.as_ref();

        // Create directories if needed
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create DB directory")?;
        }
        std::fs::create_dir_all(index_path).context("Failed to create index directory")?;

        // Initialize SQLite
        let conn = Connection::open(db_path).context("Failed to open SQLite")?;
        Self::init_schema(&conn)?;

        // Initialize Tantivy index
        let (index, schema, id_field, content_field, namespace_field, tags_field, memory_type_field, created_at_field) = 
            Self::init_index(index_path)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            index,
            schema,
            id_field,
            content_field,
            namespace_field,
            tags_field,
            memory_type_field,
            created_at_field,
        })
    }

    /// Initialize SQLite schema
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                embedding BLOB,
                namespace TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                metadata TEXT NOT NULL DEFAULT '{}',
                memory_type TEXT NOT NULL DEFAULT 'fact',
                evict_to_training INTEGER NOT NULL DEFAULT 0,
                is_persistent INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )
        .context("Failed to create memories table")?;

        // Create indices
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_namespace ON memories(namespace)",
            [],
        )
        .context("Failed to create namespace index")?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at)",
            [],
        )
        .context("Failed to create created_at index")?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_memory_type ON memories(memory_type)",
            [],
        )
        .context("Failed to create memory_type index")?;

        Ok(())
    }

    /// Initialize Tantivy index
    fn init_index(index_path: &Path) -> Result<(Index, Schema, Field, Field, Field, Field, Field, Field)> {
        let mut schema_builder = Schema::builder();
        
        let id_field = schema_builder.add_text_field("id", STRING | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let namespace_field = schema_builder.add_text_field("namespace", STRING | STORED);
        let tags_field = schema_builder.add_text_field("tags", TEXT | STORED);
        let memory_type_field = schema_builder.add_text_field("memory_type", STRING | STORED);
        let created_at_field = schema_builder.add_text_field("created_at", STRING | STORED);
        
        let schema = schema_builder.build();

        // Open or create index
        let index = if index_path.exists() && index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path).context("Failed to open index")?
        } else {
            Index::create_in_dir(index_path, schema.clone())
                .context("Failed to create index")?
        };

        Ok((
            index,
            schema,
            id_field,
            content_field,
            namespace_field,
            tags_field,
            memory_type_field,
            created_at_field,
        ))
    }

    /// Store a memory
    pub fn store(&self, memory: &Memory) -> Result<()> {
        // Store in SQLite
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT INTO memories (id, content, embedding, namespace, tags, metadata, memory_type, evict_to_training, is_persistent, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    memory.id,
                    memory.content,
                    serde_json::to_string(&memory.embedding).ok(),
                    memory.namespace,
                    serde_json::to_string(&memory.tags)?,
                    serde_json::to_string(&memory.metadata)?,
                    serde_json::to_string(&memory.memory_type)?.trim_matches('"'),
                    memory.evict_to_training as i32,
                    memory.is_persistent as i32,
                    memory.created_at.to_rfc3339(),
                    memory.updated_at.to_rfc3339(),
                ],
            )
            .context("Failed to store memory")?;
        }

        // Index in Tantivy
        self.index_memory(memory)?;

        Ok(())
    }

    /// Index a memory in Tantivy
    fn index_memory(&self, memory: &Memory) -> Result<()> {
        let mut index_writer: IndexWriter = self.index
            .writer(50_000_000)
            .context("Failed to create index writer")?;

        let mut doc = TantivyDocument::default();
        doc.add_text(self.id_field, &memory.id);
        doc.add_text(self.content_field, &memory.content);
        doc.add_text(self.namespace_field, &memory.namespace);
        doc.add_text(self.tags_field, &memory.tags.join(" "));
        doc.add_text(self.memory_type_field, &serde_json::to_string(&memory.memory_type)?.trim_matches('"'));
        doc.add_text(self.created_at_field, &memory.created_at.to_rfc3339());

        index_writer.add_document(doc).context("Failed to add document")?;
        index_writer.commit().context("Failed to commit index")?;

        Ok(())
    }

    /// Update a memory
    pub fn update(&self, memory: &Memory) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE memories SET content = ?1, namespace = ?2, tags = ?3, metadata = ?4, 
             memory_type = ?5, evict_to_training = ?6, is_persistent = ?7, updated_at = ?8
             WHERE id = ?9",
            params![
                memory.content,
                memory.namespace,
                serde_json::to_string(&memory.tags)?,
                serde_json::to_string(&memory.metadata)?,
                serde_json::to_string(&memory.memory_type)?.trim_matches('"'),
                memory.evict_to_training as i32,
                memory.is_persistent as i32,
                memory.updated_at.to_rfc3339(),
                memory.id,
            ],
        )
        .context("Failed to update memory")?;

        Ok(())
    }

    /// Delete a memory
    pub fn delete(&self, id: &str) -> Result<()> {
        // Delete from SQLite
        {
            let conn = self.conn.lock();
            conn.execute("DELETE FROM memories WHERE id = ?1", params![id])
                .context("Failed to delete memory")?;
        }

        // Delete from Tantivy (by recreating without that doc - simplified approach)
        // For production, you'd want proper doc deletion

        Ok(())
    }

    /// Get a memory by ID
    pub fn get(&self, id: &str) -> Result<Option<Memory>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT * FROM memories WHERE id = ?1")
            .context("Failed to prepare statement")?;

        let memory = stmt
            .query_row(params![id], |row| {
                Ok(Self::row_to_memory(row))
            })
            .ok();

        Ok(memory)
    }

    /// List memories in a namespace
    pub fn list(&self, namespace: &str, limit: usize) -> Result<Vec<Memory>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT * FROM memories WHERE namespace = ?1 ORDER BY created_at DESC LIMIT ?2")
            .context("Failed to prepare statement")?;

        let memories = stmt
            .query_map(params![namespace, limit as i64], |row| {
                Ok(Self::row_to_memory(row))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(memories)
    }

    /// Query memories using Tantivy text search
    pub fn query(&self, _embedding: &[f32], namespace: &str, limit: usize, _min_score: f32) -> Result<Vec<QueryResult>> {
        // Use Tantivy for text-based search
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .context("Failed to create reader")?;

        let searcher = reader.searcher();
        
        let query_parser = QueryParser::for_index(&self.index, vec![self.content_field, self.tags_field]);
        
        // Search for namespace:prefix to filter by namespace
        let query_str = format!("namespace:{}", namespace);
        let query = query_parser.parse_query(&query_str).unwrap_or_else(|_| {
            // Fallback to just namespace
            Box::new(tantivy::query::AllQuery)
        });

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))
            .context("Search failed")?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                let id = doc.get_first(self.id_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                
                if let Ok(Some(memory)) = self.get(&id) {
                    results.push(QueryResult {
                        memory,
                        score: 1.0, // Simplified - real impl would use vector similarity
                    });
                }
            }
        }

        Ok(results)
    }

    /// Convert a SQLite row to a Memory
    fn row_to_memory(row: &rusqlite::Row) -> Memory {
        let content: String = row.get(1).unwrap_or_default();
        let namespace: String = row.get(3).unwrap_or_default();
        let tags_str: String = row.get(4).unwrap_or_else(|_| "[]".to_string());
        let metadata_str: String = row.get(5).unwrap_or_else(|_| "{}".to_string());
        let memory_type_str: String = row.get(6).unwrap_or_else(|_| "\"fact\"".to_string());
        let created_at_str: String = row.get(9).unwrap_or_default();
        let updated_at_str: String = row.get(10).unwrap_or_default();

        let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
        let metadata: std::collections::HashMap<String, serde_json::Value> = 
            serde_json::from_str(&metadata_str).unwrap_or_default();
        let memory_type: MemoryType = serde_json::from_str(&memory_type_str).unwrap_or_default();
        
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        Memory {
            id: row.get(0).unwrap_or_default(),
            content,
            embedding: Vec::new(), // Not stored in SQLite
            namespace,
            tags,
            metadata,
            created_at,
            updated_at,
            memory_type,
            evict_to_training: row.get::<_, i32>(7).unwrap_or(0) != 0,
            is_persistent: row.get::<_, i32>(8).unwrap_or(0) != 0,
        }
    }

    /// Get statistics about stored memories
    pub fn stats(&self) -> Result<MemoryStats> {
        let conn = self.conn.lock();
        
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
            .unwrap_or(0);

        let by_namespace: Vec<(String, i64)> = {
            let mut stmt = conn
                .prepare("SELECT namespace, COUNT(*) FROM memories GROUP BY namespace")
                .unwrap();
            stmt.query_map([], |row| {
                Ok((row.get(0).unwrap_or_default(), row.get(1).unwrap_or(0)))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
        };

        let by_type: Vec<(String, i64)> = {
            let mut stmt = conn
                .prepare("SELECT memory_type, COUNT(*) FROM memories GROUP BY memory_type")
                .unwrap();
            stmt.query_map([], |row| {
                Ok((row.get(0).unwrap_or_default(), row.get(1).unwrap_or(0)))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
        };

        Ok(MemoryStats {
            total: total as usize,
            by_namespace,
            by_type,
        })
    }
}

/// Memory statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryStats {
    pub total: usize,
    pub by_namespace: Vec<(String, i64)>,
    pub by_type: Vec<(String, i64)>,
}
