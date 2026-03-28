# Implementation Plan

## Current Status

### ✅ IMPLEMENTED

| Component | Status | Notes |
|-----------|--------|-------|
| **API Server** | ✅ Working | All endpoints (chat, memories, training) |
| **LLM Client** | ✅ Working | Calls llama.cpp for chat |
| **Session Manager** | ✅ Working | In-memory sessions |
| **Memory Store** | ✅ Working | SQLite + Tantivy |
| **Memory Eviction** | ✅ Implemented | Policy-based eviction logic |
| **Embedding Client** | ✅ Working | Calls Ollama API (falls back to hash-based) |
| **Python CLI** | ✅ Working | Calls Agent API |

### ⚠️ STUBBED / NOT INTEGRATED

| Component | Status | What Needs Work |
|-----------|--------|-----------------|
| **Conversation → Memory** | ❌ Not integrated | Agent doesn't store conversations in memory |
| **Session Review Service** | ❌ Not implemented | No background service to analyze sessions |
| **Memory Eviction Service** | ❌ Not implemented | No background service to run eviction |
| **Search Learning Service** | ❌ Not implemented | No service to research topics |
| **Training Pipeline** | ⚠️ Partial | API exists, no actual training |
| **Reasoning Engine** | ⚠️ Partial | Simple text generation, not calling LLM |
| **Tools (search, bash, files)** | ❌ Not integrated | Defined but not used in chat |

---

## Phase 1: Core Memory Integration (Priority)

### 1.1 Store Conversations in Memory
**Current:** Agent stores sessions in-memory only
**Needed:** Store user/assistant exchanges in SQLite/Tantivy

```
src/agent/mod.rs - chat() method
```

**Changes:**
- [ ] After each chat exchange, store messages in memory store
- [ ] Use embeddings to enable semantic search
- [ ] Tag memories as `Conversation` type

### 1.2 Embedding Service
**Current:** Uses Ollama API (localhost:11434) or falls back to hash
**Needed:** llama.cpp doesn't have embeddings endpoint

**Options:**
1. Use Ollama for embeddings (requires running Ollama separately)
2. Use a dedicated embedding service (sentence-transformers)
3. Use hash-based (degraded quality but works)

**Recommendation:** Add option in config to specify embedding endpoint

### 1.3 Memory Retrieval in Chat
**Current:** Messages go straight to LLM
**Needed:** Retrieve relevant memories before calling LLM

**Changes:**
- [ ] Query memory store with user's message
- [ ] Add relevant memories to context
- [ ] Track which memories were used

---

## Phase 2: Background Services

### 2.1 Session Review Service
**Trigger:** On session end (after `end_current_session()`)
**Purpose:** Analyze session for training data

**Actions:**
- [ ] Implement `SessionReviewService` struct
- [ ] Analyze conversation for facts vs concepts
- [ ] Generate training examples from good conversations
- [ ] Move concepts to training namespace
- [ ] Delete transient conversation logs

### 2.2 Memory Eviction Service
**Trigger:** Periodic (e.g., every hour) or on TTL expiration
**Purpose:** Manage memory lifecycle

**Actions:**
- [ ] Implement `MemoryEvictionService`
- [ ] Query retrieval namespace for expired memories
- [ ] Apply `MemoryEviction` policy
- [ ] Move concepts to training, delete transient

### 2.3 Training Service
**Trigger:** Cron schedule (e.g., 2 AM daily)
**Purpose:** Fine-tune model with accumulated data

**Actions:**
- [ ] Implement actual training using unsloth
- [ ] Use training examples from memory
- [ ] Generate LoRA adapter
- [ ] Update model registry

---

## Phase 3: Tool Integration

### 3.1 Tool Registry
**Status:** Defined but not integrated

**Changes:**
- [ ] Wire tools into Agent chat flow
- [ ] Add tool calls to LLM prompt
- [ ] Handle tool results

### 3.2 Search Tool
**Needed:** Web search via SearXNG

### 3.3 Bash Tool
**Needed:** Command execution (with restrictions)

### 3.4 File Tools
**Needed:** Read/write files

---

## Phase 4: Reasoning

### 4.1 Reasoning Engine Integration
**Current:** Returns placeholder text
**Needed:** Actually call LLM for reasoning

**Changes:**
- [ ] Make `think()` call LLM with reasoning prompt
- [ ] Stream reasoning to client
- [ ] Attach reasoning to response

---

## Detailed Tasks

### Task 1: Store Conversation in Memory
```rust
// In src/agent/mod.rs - chat() method
pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse, AgentError> {
    // ... existing code ...
    
    // NEW: Store conversation in memory
    for msg in &messages {
        let mut memory = Memory::new(msg.content.clone(), "retrieval".to_string());
        memory.memory_type = MemoryType::Conversation;
        self.memory_store.store(&memory)?;
    }
    
    // ... rest of code ...
}
```

### Task 2: Retrieve Memories for Context
```rust
// In src/agent/mod.rs - chat() method
pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse, AgentError> {
    // Get last user message for retrieval
    let query = messages.last()
        .filter(|m| m.role == Role::User)
        .map(|m| m.content.clone());
    
    // Query memories if enabled
    if self.agent_config.enable_memory {
        if let Some(q) = &query {
            let embedding = self.embedding_client.embed(q).await?;
            let results = self.memory_store.query(&embedding, "retrieval", 5, 0.6)?;
            
            // Add to context
            for result in results {
                context.push(Message::system(format!(
                    "Related memory: {}",
                    result.memory.content
                )));
            }
        }
    }
    
    // ... rest of code ...
}
```

### Task 3: Session Review Service
```rust
// In src/services/session_review.rs (new file)
pub struct SessionReviewService {
    memory_store: Arc<SqliteMemoryStore>,
    session_manager: Arc<SessionManager>,
}

impl SessionReviewService {
    pub async fn process_session(&self, session: &Session) -> Result<()> {
        // Analyze conversation quality
        // Extract facts vs concepts
        // Generate training examples
        // Move to appropriate namespace
    }
}
```

---

## Configuration Additions

```toml
[memory]
storage_path = "~/.agi/memory"
retrieval_ttl_hours = 24

[embedding]
# Ollama for embeddings (separate from LLM)
endpoint = "http://localhost:11434"
model = "nomic-embed-text"

[services]
session_review_enabled = true
eviction_enabled = true
training_enabled = true
```

---

## Testing Checklist

- [ ] Chat stores messages in memory
- [ ] `/memories` endpoint returns stored conversations
- [ ] Session end triggers review
- [ ] Eviction moves expired memories
- [ ] Training generates LoRA adapter
- [ ] Tools can be called during chat
