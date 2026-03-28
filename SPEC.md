# AGI Agent - Product Specification

**Version:** 0.1.0  
**Date:** 2026-03-27  
**Status:** Draft

## Executive Summary

A Rust-native AGI agent system that combines extensive memory management with reinforcement learning to create an agent that learns from interactions. The system provides seamless chat-based interaction while performing background learning, memory consolidation, and model training.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        User Interface Layer                         │
│                    (Chat API - OpenAI Compatible)                    │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Session Layer                               │
│         (Session Management, Message Handling, Streaming)            │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Agent Core Layer                                │
│    ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐   │
│    │  Memory     │  │  Reasoning  │  │   Tool Execution        │   │
│    │  Retrieval  │  │  Engine     │  │   (Search, Bash, etc)   │   │
│    └─────────────┘  └─────────────┘  └─────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Background Services                            │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │ Session Review    │  │ Search Learning  │  │ Memory Eviction  │  │
│  │ (Memory Analysis) │  │ (Topic Expansion)│  │ (Training Data)  │  │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Training Pipeline                               │
│    ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐  │
│    │ Training Data     │  │ RL Training      │  │ Model Registry │  │
│    │ Accumulator       │  │ (GRPO/PPO)      │  │ (Versioning)   │  │
│    └──────────────────┘  └──────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Data Layer                                     │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐  │
│  │ Short-term Memory │  │ Long-term Memory │  │ Training Data  │  │
│  │ (chroma-sqlite)   │  │ (chroma-sqlite)  │  │ (File-based)  │  │
│  └──────────────────┘  └──────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Chat API Server

OpenAI-compatible chat completions API that provides seamless interaction:

- `POST /v1/chat/completions` - Main chat endpoint with streaming support
- `GET /v1/models` - List available models
- WebSocket support for real-time streaming
- Session management with automatic context window optimization

### 2. Memory System

**Short-term Memory (Retrieval)**
- Chroma-based vector storage for fast semantic search
- Namespace: `retrieval` for immediate context needs
- Stores: conversation turns, specific facts, document content
- Automatic embedding with configurable model (Ollama/nomic-embed-text default)
- TTL: Session-based, evicted after session review

**Long-term Memory (Training)**
- Separate chroma namespace for accumulated learning
- Namespace: `training` for knowledge that should persist
- Stores: learned concepts, expanded understanding, training examples
- Never evicted (accumulates over time)

**Memory Eviction Strategy**
- **Evict to training**: Conceptual knowledge, learned patterns
- **Evict from system**: Specific facts (birthdays, document content), transient data
- Sessions trigger evaluation of what should persist vs. be learned

### 3. Session Review (Background Process)

Triggered at end of each chat session:

1. **Memory Analysis**
   - Identify facts that should be evicted (specific data, transient info)
   - Identify concepts that should be learned (generalizations, patterns)
   - Check for gaps in understanding

2. **Quality Scoring**
   - Novelty: Is this new information?
   - Utility: Will this be useful in future conversations?
   - Generalizability: Can this be abstracted to broader knowledge?

3. **Actions**
   - Move useful memories to training namespace
   - Generate training examples from session
   - Flag topics for further research

### 4. Search Learning (Background Process)

Triggered when agent identifies knowledge gaps:

1. **Gap Detection**
   - Agent requests search for topics it doesn't understand
   - Confidence scoring on responses
   - Follow-up question generation

2. **Learning Pipeline**
   - Search for relevant information using SearXNG
   - Fetch and summarize content
   - Extract key concepts
   - Add to training memory

3. **Knowledge Integration**
   - Create training examples from learned content
   - Update memory embeddings
   - Flag for RL training

### 5. RL Training Pipeline

Overnight training process:

1. **Data Collection**
   - Aggregate training examples from memory
   - Filter low-quality or redundant examples
   - Format for GRPO/PPO training

2. **Model Training**
   - Use candle (Rust ML) or call Python for training
   - GRPO implementation for format rewards
   - Preference learning for response quality

3. **Model Registry**
   - Version each trained model
   - Track performance metrics
   - Automatic rollback capability
   - Hot-swap to new model

### 6. Tool System

**Built-in Tools:**
- `search` - Web search via SearXNG
- `fetch` - Webpage content extraction
- `fetch_image` - Fetch images from URLs or local files for vision model analysis
- `summarize` - AI-powered content summarization
- `bash` - Command execution
- `read` - File reading
- `write` - File writing

**Tool Integration:**
- Tools exposed via OpenAI function calling schema
- Results automatically added to memory
- Tool use logged for training examples

### 7. Multi-modal Support ⭐ NEW

The agent supports multi-modal content including images and audio:

**Content Types:**
- Text content (default)
- Image URLs
- Image base64 data
- Audio URLs
- Audio base64 data

**API Support:**
Messages can include content parts array for multi-modal input:
```json
{
  "role": "user",
  "content": [
    {"type": "text", "text": "What is in this image?"},
    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
  ]
}
```

**Image Tool:**
The `fetch_image` tool can:
- Fetch images from URLs
- Read local image files
- Return base64-encoded data for vision model processing
- Return metadata (size, media type)

## Data Models

### Memory

```rust
struct Memory {
    id: String,
    content: String,
    embedding: Vec<f32>,
    namespace: String,
    tags: Vec<String>,
    metadata: HashMap<String, Value>,
    created_at: DateTime,
    updated_at: DateTime,
    memory_type: MemoryType,  // Fact, Concept, Conversation, ToolResult
    evict_to_training: bool,
    is_persistent: bool,
}
```

### Session

```rust
struct Session {
    id: String,
    user_id: Option<String>,
    messages: Vec<Message>,
    memories: Vec<String>,  // Memory IDs used in session
    created_at: DateTime,
    ended_at: Option<DateTime>,
    status: SessionStatus,
}
```

### Message

```rust
struct Message {
    id: String,
    role: Role,  // User, Assistant, System
    content: String,           // Text content (backward compatible)
    content_parts: Vec<ContentPart>,  // Multi-modal content
    tool_calls: Option<Vec<ToolCall>>,
    tool_results: Option<Vec<ToolResult>>,
    memory_refs: Vec<String>,  // Memories used in response
    reasoning: Option<String>,
}

// Multi-modal content part
enum ContentPart {
    Text { text: String },
    ImageUrl { url: String, detail: Option<String> },
    ImageBase64 { data: String, media_type: Option<String> },
    AudioUrl { url: String },
    AudioBase64 { data: String, media_type: Option<String> },
}
```

### TrainingExample

```rust
struct TrainingExample {
    id: String,
    prompt: String,
    completion: String,
    reasoning: String,
    reward: f32,
    source: TrainingSource,  // Session, Search, Manual
    created_at: DateTime,
    quality_score: f32,
    used_in_training: bool,
}
```

## Configuration

```yaml
# agent.toml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

model:
  base_url: "http://localhost:11434"  # Ollama
  name: "qwen3:8b"
  embedding_model: "nomic-embed-text"
  embedding_dim: 768
  
memory:
  storage_path: ".agent/memory"
  retrieval_ttl_hours: 24
  default_namespace: "retrieval"
  min_similarity: 0.6
  query_limit: 10

search:
  instance: "https://search.butler.ooo"
  timeout: 30
  
training:
  enabled: true
  schedule: "0 2 * * *"  # 2 AM daily
  model: "qwen3:8b"
  output_path: ".agent/models"
  batch_size: 4
  steps: 500
  
summarization:
  provider: "openai"  # openai, anthropic, local
  api_key: ""
  model: "gpt-4o-mini"
```

## API Endpoints

### Chat Completions

```bash
# Synchronous chat
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ],
    "stream": false
  }'

# Streaming chat
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ],
    "stream": true
  }'
```

### Memory Management

```bash
# Query memories
curl http://localhost:8080/memories/query \
  -d '{"query": "What did I learn about Rust?"}'

# Store memory
curl -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "User prefers dark mode", "tags": ["preference"]}'

# List memories
curl http://localhost:8080/memories

# Delete memory
curl -X DELETE http://localhost:8080/memories/{id}
```

### Training

```bash
# Trigger training
curl -X POST http://localhost:8080/training/trigger

# Get training status
curl http://localhost:8080/training/status

# List models
curl http://localhost:8080/models
```

### Session Management (Phase 2)

```bash
# List all sessions
curl http://localhost:8080/sessions

# Create a new session
curl -X POST http://localhost:8080/sessions \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123"}'

# Get a specific session
curl http://localhost:8080/sessions/{id}

# End a session
curl -X POST http://localhost:8080/sessions/{id}/end

# Delete a session
curl -X DELETE http://localhost:8080/sessions/{id}
```

### Self-Improvement (Phase 3)

```bash
# Get self-improvement statistics
curl http://localhost:8080/self-improve/stats

# Run self-improvement analysis
curl -X POST http://localhost:8080/self-improve/analyze \
  -H "Content-Type: application/json" \
  -d '{
    "interactions": [],
    "tool_usage": {"search": 5, "bash": 2},
    "errors": []
  }'

# Get improvements
curl http://localhost:8080/self-improve/improvements

# Apply an improvement
curl -X POST http://localhost:8080/self-improve/apply \
  -H "Content-Type: application/json" \
  -d '{"improvement_id": "uuid-here"}'

# Get current system prompt
curl http://localhost:8080/self-improve/prompt
```

### Theory of Mind (Phase 3)

```bash
# Get theory of mind statistics
curl http://localhost:8080/tom/stats

# Update user mental model
curl -X POST http://localhost:8080/tom/user \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user-123",
    "messages": [{"role": "user", "content": "How do I learn Rust?"}]
  }'

# Get user mental model
curl http://localhost:8080/tom/user

# Analyze user for response recommendations
curl -X POST http://localhost:8080/tom/analyze \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123"}'

# Update trust level
curl -X POST http://localhost:8080/tom/trust \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123", "delta": 0.1}'
```

### Model Management (Hot-Swap)

```bash
# Get current model status
curl http://localhost:8080/model/status

# Validate a model configuration
curl -X POST http://localhost:8080/model/validate \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen3:8b", "base_url": "http://localhost:11434"}'

# Hot-swap to a new model
curl -X POST http://localhost:8080/model/update \
  -H "Content-Type: application/json" \
  -d '{"model": "llama3:70b"}'

# List available models on the endpoint
curl http://localhost:8080/model/available
```

### Learned Concepts

```bash
# Get all learned concepts from training memory
curl http://localhost:8080/concepts

# Search learned concepts by semantic similarity
curl -X POST http://localhost:8080/concepts/search \
  -H "Content-Type: application/json" \
  -d '{"query": "rust programming", "limit": 10}'
```
```

## Technical Stack

- **Language**: Rust (2024 edition)
- **Web Framework**: Axum for HTTP/WebSocket
- **Database**: SQLite with Tantivy for vector search
- **ML/Embedding**: Candle for tensor operations, Ollama/llama.cpp for inference
- **Async Runtime**: Tokio
- **Serialization**: Serde with JSON
- **Configuration**: Toml, config-rs
- **CLI Parsing**: Clap
- **Logging**: Tracing, tracing-subscriber

## Implementation Status

### ✅ Phase 1 Complete (Verified 2026-03-28)
- Chat API Server with OpenAI-compatible endpoints ✅
- Memory System (short-term and long-term) ✅
- Session Management ✅
- Tool System (search, bash, read, write) ✅
- Reasoning Engine with LLM integration ✅
- Training Pipeline with GRPO ✅

**Build Status:** Release build successful (18MB binary)
**Test Status:** 174 tests passing (verified 2026-03-29)
**Warnings:** 29 (intentional public API items)

### Phase 2 ✅ COMPLETE
- [x] Multi-modal support (images, audio)
- [x] Persistent user sessions ⭐ NEW
- [x] Team of agents with shared memory
- [x] External knowledge base integration ⭐ NEW

### Phase 3 ✅ COMPLETE
- [x] Continuous learning (online RL) ⭐ NEW (2026-03-28)
- [x] Curiosity-driven exploration ⭐ NEW (2026-03-28)
- [x] Self-improvement through code generation ⭐ NEW (2026-03-28)
- [x] Theory of mind modeling ⭐ NEW (2026-03-28)

## Success Criteria

**Note:** The following criteria require real-world testing to verify. The implementation is complete but ongoing validation is needed.

### Implemented Features ✅
- [x] Hot-swap models without service interruption - Arc<RwLock<LlmClient>> enables runtime model updates
- [x] Agent can learn new concepts from search - `/concepts` and `/concepts/search` endpoints implemented

### Testing Required ⏳
- [ ] Seamless chat interaction indistinguishable from standard LLM
- [ ] Memory retrieval improves response quality over time
- [ ] Overnight training improves agent capabilities
- [ ] No data loss during memory eviction
