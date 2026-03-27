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
- `summarize` - AI-powered content summarization
- `bash` - Command execution
- `read` - File reading
- `write` - File writing

**Tool Integration:**
- Tools exposed via OpenAI function calling schema
- Results automatically added to memory
- Tool use logged for training examples

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
    content: Content,
    tool_calls: Option<Vec<ToolCall>>,
    tool_results: Option<Vec<ToolResult>>,
    memory_refs: Vec<String>,  // Memories used in response
    reasoning: Option<String>,
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

## Technical Stack

- **Language**: Rust (2021 edition)
- **Web Framework**: Axum for HTTP/WebSocket
- **Database**: SQLite with chrome (chroma fork for Rust)
- **ML/Embedding**: Candle for tensor operations, Ollama for inference
- **Async Runtime**: Tokio
- **Serialization**: Serde with JSON
- **Configuration**: Toml, config-rs
- **CLI Parsing**: Clap
- **Logging**: Tracing, tracing-subscriber

## Future Enhancements

### Phase 2
- [ ] Multi-modal support (images, audio)
- [ ] Persistent user sessions
- [ ] Team of agents with shared memory
- [ ] External knowledge base integration

### Phase 3
- [ ] Continuous learning (online RL)
- [ ] Curiosity-driven exploration
- [ ] Self-improvement through code generation
- [ ] Theory of mind modeling

## Success Criteria

- [ ] Seamless chat interaction indistinguishable from standard LLM
- [ ] Memory retrieval improves response quality over time
- [ ] Agent can learn new concepts from search
- [ ] Overnight training improves agent capabilities
- [ ] No data loss during memory eviction
- [ ] Hot-swap models without service interruption
