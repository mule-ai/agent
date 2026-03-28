# Implementation Plan

## Current Status

### ✅ IMPLEMENTED AND VERIFIED (2026-03-29)

**Build Status:** ✅ Release build successful (18MB agent binary)
**Test Status:** ✅ 174 tests passing
**Warnings:** ✅ 29 warnings (reduced from 34)
**Documentation:** ✅ README updated with all API endpoints (2026-03-29)

| Component | Status | Notes |
|-----------|--------|-------|
| **API Server** | ✅ Working | All endpoints (chat, memories, training, concepts, model, sessions, self-improve, tom, knowledge) |
| **LLM Client** | ✅ Working | Calls llama.cpp with tool support |
| **Session Manager** | ✅ Working | In-memory sessions |
| **Session Store** | ✅ Working | SQLite-based persistent sessions |
| **Memory Store** | ✅ Working | SQLite + Tantivy |
| **Memory Eviction** | ✅ Implemented | Policy-based eviction logic |
| **Embedding Client** | ✅ Working | Calls Ollama API (falls back to hash-based) |
| **Python CLI** | ✅ Working | Calls Agent API |
| **Session Review Service** | ✅ Implemented | Analyzes sessions, extracts facts/concepts |
| **Memory Eviction Service** | ✅ Implemented | Background service for TTL-based eviction |
| **Search Learning Service** | ✅ Implemented | Research topics using SearXNG |
| **Tool System** | ✅ Implemented | Search, Bash, Read, Write tools |
| **Conversation → Memory** | ✅ Integrated | Agent stores conversations in memory |
| **Memory Retrieval in Chat** | ✅ Integrated | Retrieves relevant memories before calling LLM |
| **Tool Registry** | ✅ Working | Tools registered and available |
| **Training Pipeline** | ✅ Implemented | GRPO reward functions, data accumulator, model registry |
| **Reasoning Engine** | ✅ Improved | Now uses LLM for actual reasoning (with fallback) |
| **Tool Execution in Chat** | ✅ Integrated | LLM client supports function calling |
| **Model Hot-Swap** | ✅ Working | Runtime model switching without restart |
| **Learned Concepts** | ✅ Implemented | Queryable concepts from training memory |
| **Multi-Modal** | ✅ Implemented | Image/audio support with base64 |
| **External Knowledge** | ✅ Implemented | Wikipedia, arXiv, web fetch APIs |
| **Online Learning** | ✅ Implemented | Continuous RL from tool interactions |
| **Curiosity-Driven** | ✅ Implemented | Curiosity-driven exploration |
| **Self-Improvement** | ✅ Implemented | Code pattern analysis and improvements |
| **Theory of Mind** | ✅ Implemented | User mental state modeling |
| **Agent Teams** | ✅ Implemented | Multi-agent with shared memory |

---

## Phase 1: Core Memory Integration (Priority) ✅

### 1.1 Store Conversations in Memory ✅
**Status:** Implemented in `src/agent/mod.rs`

**Changes:**
- [x] After each chat exchange, store messages in memory store
- [x] Use embeddings to enable semantic search
- [x] Tag memories as `Conversation` type

### 1.2 Embedding Service ✅
**Status:** Uses Ollama API (localhost:11434) or falls back to hash

**Options:**
1. Use Ollama for embeddings (requires running Ollama separately)
2. Use a dedicated embedding service (sentence-transformers)
3. Use hash-based (degraded quality but works)

**Recommendation:** Add option in config to specify embedding endpoint

### 1.3 Memory Retrieval in Chat ✅
**Status:** Implemented in `src/agent/mod.rs`

**Changes:**
- [x] Query memory store with user's message
- [x] Add relevant memories to context
- [x] Track which memories were used

---

## Phase 2: Background Services ✅

### 2.1 Session Review Service ✅
**Status:** Implemented in `src/services/session_review.rs`

**Actions:**
- [x] Implement `SessionReviewService` struct
- [x] Analyze conversation for facts vs concepts
- [x] Generate training examples from good conversations
- [x] Move concepts to training namespace
- [x] Delete transient conversation logs

### 2.2 Memory Eviction Service ✅
**Status:** Implemented in `src/services/memory_eviction.rs`

**Actions:**
- [x] Implement `MemoryEvictionService`
- [x] Query retrieval namespace for expired memories
- [x] Apply `MemoryEviction` policy
- [x] Move concepts to training, delete transient

### 2.3 Training Service ✅
**Status:** Implemented in `src/training/mod.rs` and `src/services/batch_training.rs`

**Actions:**
- [x] Implement training pipeline with GRPO reward functions
- [x] Use training examples from memory
- [x] Generate LoRA adapter (via Python/unsloth integration)
- [x] Update model registry
- [x] **NEW:** BatchTrainingService for integrating training module with API
- [x] **NEW:** Batch training API endpoints for training management

---

## Phase 3: Tool Integration ✅

### 3.1 Tool Registry ✅
**Status:** Fully integrated into agent chat flow

**Changes:**
- [x] Wire tools into Agent chat flow
- [x] Add tool calls to LLM prompt via `chat_with_tools()`
- [x] Handle tool results

### 3.2 Search Tool ✅
**Status:** Implemented in `src/tools/search.rs`

### 3.3 Bash Tool ✅
**Status:** Implemented in `src/tools/bash.rs`

### 3.4 File Tools ✅
**Status:** Implemented in `src/tools/read.rs` and `src/tools/write.rs`

---

## Phase 4: Reasoning ✅

### 4.1 Reasoning Engine Integration ✅
**Status:** Uses LLM for actual reasoning

**Changes:**
- [x] Make `think()` call LLM with reasoning prompt
- [x] Stream reasoning to client (via chat response)
- [x] Attach reasoning to response
- [x] Fallback to simple analysis if LLM fails

---

## Detailed Tasks

### Task 1: Store Conversation in Memory ✅
Implemented in `src/agent/mod.rs` - `store_conversations()` method
- Stores messages as memories with type `Conversation`
- Tags memories with role info

### Task 2: Retrieve Memories for Context ✅
Implemented in `src/agent/mod.rs` - `retrieve_memories()` method
- Generates embedding for last user message
- Queries memory store with similarity threshold
- Builds context with relevant memories

### Task 3: Session Review Service ✅
Implemented in `src/services/session_review.rs`
- Analyzes conversation for facts vs concepts
- Generates training examples from good conversations
- Extracts topics for research

### Task 4: LLM Client with Tool Support ✅
Implemented in `src/agent/llm.rs` - `chat_with_tools()` method
- Sends tools to LLM via OpenAI function calling format
- Parses tool calls from response
- Returns content and tool calls separately

### Task 5: Reasoning Engine LLM Integration ✅
Implemented in `src/agent/reasoning.rs` - `llm_think()` method
- Creates reasoning prompt from conversation context
- Calls LLM for actual reasoning
- Falls back to simple analysis on failure

### Task 6: Training Pipeline ✅
Implemented in `src/training/mod.rs` and `src/services/batch_training.rs`
- `TrainingDataAccumulator`: Collects and filters training examples
- `ModelRegistry`: Manages model versions
- `TrainingPipeline`: Orchestrates training via Python/unsloth
- `BatchTrainingService`: Bridges training module with API (NEW)
- GRPO reward functions: format, helpfulness, combined rewards
- Batch training API endpoints: status, collect, add, run, export, clear (NEW)

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

- [x] Chat stores messages in memory
- [x] `/memories` endpoint returns stored conversations
- [x] Session review service analyzes conversations
- [x] Memory eviction service processes expired memories
- [x] Search learning service researches topics
- [x] Tools can be called during chat (LLM function calling implemented)
- [x] Training pipeline implemented (requires unsloth for actual training)

---

## Phase 2 Enhancements ✅ COMPLETE
- [x] Multi-modal support (images, audio) ⭐ NEW (2026-03-28)
  - ContentPart enum supporting text, images, audio
  - Image URL and base64 support
  - Audio URL and base64 support
  - fetch_image tool for fetching images from URLs/files
  - API updated to handle multi-modal content parts
- [x] Persistent user sessions ⭐ NEW (2026-03-28)
  - SQLite-based session persistence
  - New `/sessions` API endpoints
  - Fallback to in-memory if SQLite unavailable
- [x] Team of agents with shared memory ⭐ NEW (2026-03-28)
- [x] External knowledge base integration ⭐ NEW (2026-03-28)
  - Wikipedia API client for querying articles
  - ArXiv API client for academic paper search
  - Web fetcher for general web content
  - Fetch tool for retrieving web page content
  - API endpoints: `/knowledge/search`, `/knowledge/wikipedia/{title}`, `/knowledge/arxiv/{id}`, `/knowledge/fetch`, `/knowledge/sources`
  - KnowledgeEntry struct with relevance scoring
  - Integration with AppState for easy access

## Phase 3 Enhancements ✅ COMPLETE
- [x] Continuous learning (online RL) ⭐ NEW (2026-03-28)
- [x] Curiosity-driven exploration ⭐ NEW (2026-03-28)
- [x] Self-improvement through code generation ⭐ NEW (2026-03-28)
  - [x] Analyze code changes from searches (via `analyze_code_from_search`)
  - [x] Identify patterns for improvement (via `detect_code_patterns`)
  - [x] Generate improvement suggestions (via `generate_improvement_suggestions`)
  - [x] Apply improvements to agent code (via `apply_code_improvement`)
  - [x] Track improvement history (via `ImprovementHistoryEntry`)
- [x] Theory of mind modeling ⭐ NEW (2026-03-28)

---

## Training Data Generation - KNOWN LIMITATION

### Issue: Training Data Pipeline Not Fully Wired

The training system is designed to work as follows:

```
┌─────────────────┐    ┌─────────────────────┐    ┌──────────────────┐
│  Chat Sessions  │───▶│ SessionReviewService│───▶│ Training Examples│
└─────────────────┘    │  (analyze_session)  │    └────────┬─────────┘
                       └─────────────────────┘             │
                                                             ▼
┌─────────────────┐    ┌─────────────────────┐    ┌──────────────────┐
│   BatchTraining │◀───│  Training Examples   │◀───│  Move to "train" │
│   Service       │    │  (accumulator)       │    │  namespace       │
└────────┬────────┘    └─────────────────────┘    └──────────────────┘
         │
         ▼
┌─────────────────┐
│ Python/unsloth  │
│ Training Script │
└─────────────────┘
```

### What's Implemented:
- ✅ `SessionReviewService` - Analyzes sessions, extracts facts/concepts, generates training examples
- ✅ `TrainingDataAccumulator` - Collects and filters training examples by quality
- ✅ `BatchTrainingService` - Bridges training module with API
- ✅ Training pipeline with GRPO rewards and Python/unsloth integration
- ✅ API endpoints: `/training/trigger`, `/training/status`, `/training/batch/*`

### What's NOT Working (as of 2026-03-29):
- ❌ **No automatic session review** - `SessionReviewService` is implemented but not called automatically after sessions
- ❌ **Training examples not stored to disk** - Generated examples exist in memory but aren't persisted for batch training
- ❌ **No connection between SessionReviewService and BatchTrainingService** - These two services don't communicate
- ❌ **"training" namespace is empty** - No memories exist in the training namespace (all are in "retrieval")
- ❌ **Search learning doesn't generate training examples** - Research happens but isn't saved for training
- ❌ **No scheduled batch training** - Training must be triggered manually via API
- ❌ **Training data quality is low** - Regex-based extraction produces basic examples

### Implementation Status:
- [ ] TASK 1: Wire session review to session end
- [ ] TASK 2: Persist training examples to disk
- [ ] TASK 3: Wire search learning to generate training data
- [ ] TASK 4: Implement curiosity-driven gap research
- [ ] TASK 5: Create scheduled batch training job
- [ ] TASK 6: Enhance training data quality with LLM

### Required Implementation Tasks:

#### TASK 1: Wire Session Review to Session End (HIGH)
**File:** `src/api/sessions.rs`

**Steps:**
1. Modify `end_session()` to accept `State<Arc<AppState>>`
2. Get `session_review_service` from state
3. Load session messages from session store
4. Call `session_review_service.review_session(id, &messages)`
5. Store results (training examples, memories to move)

**Code Changes:**
```rust
// In end_session(), after marking session as ended:
let review_service = &state.session_review_service;
let messages = session.messages.clone();
let result = review_service.review_session(&id, &messages);

// Store training examples to batch service
for example in session_review_service.generate_training_examples(&messages) {
    state.batch_training_service.add_example(example).await;
}

// Move concept memories to training namespace
for memory in session_review_service.generate_memories(&messages) {
    if memory.evict_to_training {
        state.memory_store.store(&memory.with_namespace("training"));
    }
}
```

**Test:**
```bash
# 1. Start a conversation via API
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"messages": [{"role": "user", "content": "Tell me about Rust"}]}'

# 2. End the session
curl -X POST http://localhost:8080/sessions/{session_id}/end

# 3. Check batch training service accumulated examples
curl http://localhost:8080/training/batch/status | jq '.examples_collected'

# 4. Verify training namespace has concepts
curl http://localhost:8080/memories?namespace=training | jq '.total'
```

---

#### TASK 2: Persist Training Examples to Disk (HIGH)
**File:** `src/services/batch_training.rs`

**Steps:**
1. Add file-based storage to `BatchTrainingService`
2. Write training examples to `~/.agi/training/examples.jsonl`
3. Load examples on service initialization
4. Clear examples after successful training run

**Code Changes:**
```rust
// In BatchTrainingService
let examples_path = PathBuf::from(std::env::var("HOME").unwrap())
    .join(".agi/training/examples.jsonl");

// Load on init
fn load_examples(&self) -> Vec<TrainingExample> {
    let path = &self.examples_path;
    if path.exists() {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    } else {
        Vec::new()
    }
}

// Save after adding
fn save_examples(&self, examples: &[TrainingExample]) {
    let jsonl: String = examples.iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::create_dir_all(self.examples_path.parent().unwrap()).ok();
    std::fs::write(&self.examples_path, jsonl).ok();
}
```

**Test:**
```bash
# 1. Trigger a few session reviews
# ... (from Task 1 test)

# 2. Check examples file exists
cat ~/.agi/training/examples.jsonl | head -5 | jq '.'

# 3. Restart agent
pkill -f agent
nohup /tmp/target/release/agent > /tmp/agent.log 2>&1 &

# 4. Verify examples loaded
curl http://localhost:8080/training/batch/status | jq '.examples_collected'
```

---

#### TASK 3: Wire Search Learning to Generate Training Data (MEDIUM)
**File:** `src/services/search_learning.rs`

**Steps:**
1. After `SearchLearningService` completes research
2. Generate `TrainingExample` from learned content
3. Add to `BatchTrainingService.accumulator`

**Code Changes:**
```rust
// In SearchLearningService, after research completes:
pub async fn learn(&self, topic: &str) -> Result<TrainingExample> {
    // ... existing research logic ...
    
    // NEW: Generate training example from research
    let example = TrainingExample {
        id: Uuid::new_v4().to_string(),
        prompt: format!("Tell me about {}", topic),
        completion: research_summary,
        reasoning: format!("Researched from {} sources", sources.len()),
        reward: 0.8, // Research-derived content is high quality
        source: TrainingSource::Search,
        created_at: Utc::now(),
        quality_score: 0.8,
        used_in_training: false,
    };
    
    // Add to batch training
    if let Some(batch_service) = self.batch_training_service.as_ref() {
        batch_service.add_example(example.clone()).await;
    }
    
    Ok(example)
}
```

**Test:**
```bash
# 1. Trigger search learning
curl -X POST http://localhost:8080/services/search-learning \
  -d '{"topic": "Rust ownership model"}'

# 2. Check training examples increased
curl http://localhost:8080/training/batch/status | jq '.examples_collected'

# 3. Export and view example
curl http://localhost:8080/training/batch/export | jq '.jsonl' | head -1 | jq '.'
```

---

#### TASK 4: Implement Curiosity-Driven Gap Research (MEDIUM)
**File:** `src/agent/mod.rs`

**Steps:**
1. In chat flow, detect low-confidence responses
2. Queue topic for research via `CuriosityEngine`
2. Schedule background research via `SearchLearningService`
3. When research completes, generate training example

**Test:**
```bash
# 1. Ask a question the agent doesn't know well
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"messages": [{"role": "user", "content": "Explain quantum entanglement"}]}'

# 2. Check curiosity queue for detected gaps
curl http://localhost:8080/curiosity/gaps/pending | jq '.'

# 3. Process pending gap
curl -X POST http://localhost:8080/curiosity/explore \
  -d '{"gap_id": "..."}'

# 4. Verify training example created
curl http://localhost:8080/training/batch/status | jq '.examples_collected'
```

---

#### TASK 5: Create Scheduled Batch Training Job (MEDIUM)
**File:** `src/main.rs` or new `src/services/scheduler.rs`

**Steps:**
1. Add cron-based scheduler using `tokio-cron-scheduler`
2. Run `batch_training_service.train()` at configured time (default: 2 AM)
3. Log results and update model registry

**Test:**
```bash
# 1. Manually trigger batch training
curl -X POST http://localhost:8080/training/batch/run

# 2. Watch logs
tail -f /tmp/agent.log | grep -i "train"

# 3. Check training completes
curl http://localhost:8080/training/status | jq '.current_job.status'

# 4. Verify model saved
ls -la ~/.agi/models/

# 5. Check model registry
curl http://localhost:8080/training/models/list | jq '.'
```

---

#### TASK 6: Enhance Training Data Quality with LLM (MEDIUM)
**File:** `src/services/session_review.rs`

**Steps:**
1. Replace regex-based extraction with LLM call
2. Prompt LLM to generate structured training pairs from conversation
3. Parse structured output for quality examples

**Test:**
```bash
# 1. Have a detailed conversation
# ... 

# 2. Export session for review
curl http://localhost:8080/sessions/{id} | jq '.messages'

# 3. Check generated training examples have better structure
curl http://localhost:8080/training/batch/export | jq '.jsonl' | jq -s '.[0]'
```

---

## Complete System Test Plan

### Phase 1: Basic Service Wiring (Before Testing)
- [ ] Complete Task 1: Session review triggers on session end
- [ ] Complete Task 2: Training examples persist to disk
- [ ] Build and restart agent

### Phase 2: Functional Tests

```bash
# TEST 1: Conversation creates training data
# 1. Start new session
SESSION=$(curl -s -X POST http://localhost:8080/sessions \
  -d '{"user_id": "test"}' | jq -r '.session.id')

# 2. Have meaningful conversation
curl -X POST http://localhost:8080/v1/chat/completions \
  -d "{\"session_id\": \"$SESSION\", \"messages\": [{\"role\": \"user\", \"content\": \"What are the benefits of Rust over Go?\"}]}"

# 3. End session
curl -X POST http://localhost:8080/sessions/$SESSION/end

# 4. Verify training example created
EXAMPLES=$(curl -s http://localhost:8080/training/batch/status | jq '.examples_collected')
echo "Examples: $EXAMPLES"
test $EXAMPLES -gt 0 && echo "PASS: Training examples created" || echo "FAIL: No examples"

# TEST 2: Training examples persist after restart
# 1. Kill agent
pkill -f agent

# 2. Restart agent
nohup /tmp/target/release/agent > /tmp/agent.log 2>&1 &

# 3. Check examples still there
EXAMPLES=$(curl -s http://localhost:8080/training/batch/status | jq '.examples_collected')
test $EXAMPLES -gt 0 && echo "PASS: Examples persisted" || echo "FAIL: Examples lost"

# TEST 3: Batch training runs successfully
# 1. Ensure examples exist
curl http://localhost:8080/training/batch/status | jq '.examples_collected'

# 2. Activate training environment
source ~/venv/bin/activate

# 3. Run training (this may take a while)
curl -X POST http://localhost:8080/training/batch/run

# 4. Wait for completion, check status
for i in {1..30}; do
  STATUS=$(curl -s http://localhost:8080/training/status | jq -r '.current_job.status')
  echo "Status: $STATUS"
  [ "$STATUS" = "completed" ] && break
  [ "$STATUS" = "failed" ] && echo "FAIL: Training failed" && break
  sleep 10
done

# 5. Verify model file created
ls -la ~/.agi/models/ && echo "PASS: Model saved" || echo "FAIL: No model"

# TEST 4: Hot-swap to trained model
# 1. Get model ID
MODEL_ID=$(ls ~/.agi/models/ | head -1)
[ -z "$MODEL_ID" ] && echo "SKIP: No model to test" && exit

# 2. Update model
curl -X POST http://localhost:8080/model/update \
  -d "{\"adapter\": \"~/.agi/models/$MODEL_ID\"}"

# 3. Verify chat still works
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"messages": [{"role": "user", "content": "Hello"}]}' | jq -r '.choices[0].message.content'

# 4. Check model status shows adapter
curl http://localhost:8080/model/status | jq '.adapter'

### Phase 3: Long-term Learning Tests

# TEST 5: Multiple sessions create diverse training data
for topic in "Rust programming" "Go vs Rust" "Memory safety" "Systems programming"; do
  SESSION=$(curl -s -X POST http://localhost:8080/sessions -d '{}' | jq -r '.session.id')
  curl -X POST http://localhost:8080/v1/chat/completions \
    -d "{\"session_id\": \"$SESSION\", \"messages\": [{\"role\": \"user\", \"content\": \"Tell me about $topic\"}]}"
  curl -X POST http://localhost:8080/sessions/$SESSION/end
done

# Check diversity of training data
curl http://localhost:8080/training/batch/export | jq '.line_count'
curl http://localhost:8080/training/batch/export | jq '.jsonl' | jq -r 'split("\n")[0:3]' | head -20

# TEST 6: Memory eviction moves data correctly
# 1. Wait for TTL (or manually trigger eviction)
curl -X POST http://localhost:8080/services/memory-eviction/process

# 2. Check training namespace has content
curl http://localhost:8080/memories?namespace=training | jq '.total'

# 3. Verify facts moved, concepts retained
curl http://localhost:8080/memories?namespace=retrieval | jq '.total'
```

---

## Expected Final State

After completing all tasks, the system should flow:

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         USER CONVERSATION                                │
│   User: "What is Rust?" → Agent: "Rust is a systems language..."         │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                         SESSION STORAGE                                   │
│   Store messages with session_id                                         │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼ (on session end)
┌──────────────────────────────────────────────────────────────────────────┐
│                    SESSION REVIEW SERVICE                                │
│   • Extract facts: "User likes dark themes"                              │
│   • Extract concepts: "Rust is a systems language"                       │
│   • Generate Q&A pairs from conversation                                 │
│   • Calculate quality scores                                            │
└──────────────────────────────────────────────────────────────────────────┘
                         │                    │
                         ▼                    ▼
              ┌──────────────────┐   ┌─────────────────────────┐
              │ Training Service  │   │ Memory Eviction Service │
              │ • Store examples │   │ • Move concepts to      │
              │   to disk        │   │   "training" namespace  │
              │ • Accumulate     │   │ • Delete transient     │
              └──────────────────┘   └─────────────────────────┘
                         │                    │
                         ▼                    ▼
              ┌──────────────────┐   ┌─────────────────────────┐
              │ ~/.agi/training/ │   │ Training namespace      │
              │ examples.jsonl   │   │ (learned concepts)     │
              └──────────────────┘   └─────────────────────────┘
                         │                    │
                         ▼                    │
              ┌──────────────────┐            │
              │ SCHEDULED BATCH   │            │
              │ TRAINING (2 AM)   │            │
              │ • Load JSONL      │            │
              │ • Run unsloth     │            │
              │ • Save LoRA       │            │
              └──────────────────┘            │
                         │                    │
                         ▼                    ▼
              ┌──────────────────────────────────────────┐
              │          ~/.agi/models/                  │
              │   qwen3.5-4b-20260329-020000/          │
              │   └── adapter_model.safetensors        │
              └──────────────────────────────────────────┘
                                    │
                                    ▼
              ┌──────────────────────────────────────────┐
              │        HOT-SWAP MODEL (via API)         │
              │   curl -X POST /model/update ...        │
              └──────────────────────────────────────────┘
                                    │
                                    ▼
              ┌──────────────────────────────────────────┐
              │    AGENT NOW RESPONDS WITH              │
              │    TRAINED BEHAVIOR                      │
              └──────────────────────────────────────────┘
```

### Memory Data Issue:

Current memories are stored as:
```json
{
  "content": "My favorite color is green",
  "namespace": "retrieval",
  "tags": ["role:user"],
  "memory_type": "fact"
}
```

The `BatchTrainingService.collect_from_memory()` expects:
- Memories in "training" namespace
- Structured format like `"Q: ...\nA: ..."`
- Quality scores in metadata

### References:
- SPEC.md Section: "Session Review (Background Process)" - specifies the expected behavior
- `src/services/session_review.rs` - SessionReviewService implementation
- `src/services/batch_training.rs` - BatchTrainingService implementation
- `TRAINING.md` - Environment setup for running training

---

## Success Criteria (Testing Needed)

### Prerequisites (Must Complete First)
The following tasks must be implemented before these criteria can be tested:
- [ ] TASK 1: Wire session review to session end
- [ ] TASK 2: Persist training examples to disk  
- [ ] TASK 5: Create scheduled batch training job

### Criteria Tied to Implementation Tasks ⏳

| Criterion | Test Command | Expected Result |
|-----------|-------------|-----------------|
| Session creates training data | Run TEST 1 from Complete System Test Plan | `examples_collected > 0` |
| Training data persists | Run TEST 2 | Examples survive restart |
| Batch training runs | Run TEST 3 | `training status = completed`, model file exists |
| Model hot-swap works | Run TEST 4 | Chat works with new adapter |
| Multiple sessions create diverse data | Run TEST 5 | `line_count >= 5`, diverse topics |
| Memory eviction works | Run TEST 6 | Training namespace has content |

### Previously Implemented ✅
- [x] Hot-swap models without service interruption (verified 2026-03-29)
- [x] `/concepts` and `/concepts/search` endpoints working
- [x] Individual services implemented (session_review, search_learning, etc.)

---

## Build Status (2026-03-29) ✅

All compilation errors have been fixed:
- **174 tests passing** ✅
- **Release build successful** (18MB binary) ✅
- **Warning cleanup completed** - Reduced warnings from 88 to 33 (55 fewer warnings total from start of session)

### Build Warning Notes (2026-03-29)
- Reduced warnings from 69 to 33 (36 fewer warnings)
- Remaining 33 warnings are from intentionally public API items for CLI, training, and services
- All unused internal implementation items have been cleaned up
- Dead code removed: `search_and_get`, `search_by_title`, `calculate_relevance` functions

### New Features Added (2026-03-28)
- **BatchTrainingService** (`src/services/batch_training.rs`): Integrates training module components with the services system
- **Training API Endpoints**: New batch training endpoints for collecting, adding, running, exporting training examples
- **TrainingSource::Memory**: Added Memory variant for training examples from memory

### Additional Features Added (2026-03-28 Evening)
- **Quality Filtering API**: Filter training examples by quality threshold and export as JSONL
- **Model Registry Integration**: Wired up `ModelRegistry` methods (`list_models`, `get_current_model`, `set_current_model`) to `TrainingPipeline` and BatchTrainingService
- **New API Endpoints**:
  - `POST /training/batch/filter` - Filter examples by quality threshold
  - `GET /training/models/list` - List all trained models
  - `POST /training/models/current` - Set current active model

See progress.md for details on all fixes made.
