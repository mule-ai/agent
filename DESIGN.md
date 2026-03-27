# AGI Agent Design Document

## System Overview

This document explains how the AGI Agent system is designed and how all components work together to create an intelligent agent with memory and learning capabilities.

## Key Design Principles

### 1. Rust-Native Implementation
- No WebAssembly, pure Rust
- Uses `axum` for HTTP/WebSocket server
- Uses `tantivy` for vector search
- Uses `rusqlite` for storage
- Uses `candle` for ML operations

### 2. OpenAI-Compatible API
- Drop-in replacement for existing applications
- `/v1/chat/completions` - main chat endpoint
- `/v1/models` - list available models
- Standard request/response formats

### 3. Separation of Concerns
- **Agent Core**: Conversation handling, reasoning
- **Memory System**: Storage, retrieval, eviction
- **Tools**: Search, bash, file operations
- **Services**: Background processing
- **Training**: RL pipeline

### 4. Memory Hierarchy

```
┌─────────────────────────────────────────────────────────────┐
│                    Training Data                            │
│   (Accumulated over time, used for RL training)            │
│   Namespace: "training"                                    │
│   Eviction: Never deleted                                  │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │ Evict on TTL
                              │
┌─────────────────────────────────────────────────────────────┐
│                   Retrieval Memory                          │
│   (Short-term context for conversations)                   │
│   Namespace: "retrieval"                                    │
│   Eviction: Delete or evict to training on TTL             │
└─────────────────────────────────────────────────────────────┘
```

## Component Details

### Agent Core (`src/agent/`)

#### Session Manager
- Manages conversation sessions
- Tracks messages within sessions
- Records memory references
- Handles session lifecycle (active → completed → cleaned up)

#### Reasoning Engine
- Generates thought chains
- Tracks dependencies between thoughts
- Calculates confidence scores
- Makes decisions from reasoning

#### LLM Client
- Communicates with Ollama/OpenAI-compatible APIs
- Handles both streaming and non-streaming
- Manages tool calls
- Parses reasoning content

### Memory System (`src/memory/`)

#### Memory Store
- **SQLite**: Primary storage for memory metadata
- **Tantivy**: Full-text search and filtering
- **In-memory cache**: Fast embedding lookups

#### Embedding Client
- Generates vector embeddings for memories
- Supports Ollama and OpenAI-compatible APIs
- LRU cache for recent embeddings

#### Memory Retriever
- Semantic similarity search
- Context building for prompts
- Fact/concept extraction

#### Eviction Policy
- TTL-based expiration
- Memory type evaluation
- Quality scoring
- Training data generation

### Tools (`src/tools/`)

#### Search Tool
- Uses SearXNG for private search
- Returns formatted results

#### Bash Tool
- Secure command execution
- Allowed/denied command lists
- Timeout handling

#### File Tools
- Read/write operations
- Directory listing
- Path restrictions

#### Tool Registry
- Central tool management
- Function schema generation
- Unified execution interface

### Background Services (`src/services/`)

#### Session Review Service
- Runs after session completion
- Generates training examples from conversations
- Identifies knowledge gaps
- Evaluates response quality

#### Search Learning Service
- Processes queued topics
- Searches and fetches content
- Extracts key points
- Stores in training memory

#### Memory Eviction Service
- Runs on schedule
- Processes namespace TTL
- Creates training examples from evicted memories
- Updates statistics

### Training Pipeline (`src/training/`)

#### Training Data Collection
- Aggregates examples from:
  - Session reviews
  - Evicted memories
  - Manual additions

#### GRPO Training
- Group Relative Policy Optimization
- Reward functions:
  - Format reward (proper tag usage)
  - Correctness reward (expected answers)
- LoRA fine-tuning

#### Model Registry
- Version tracking
- Performance metrics
- Hot-swap capability

## Data Flow

### Chat Request Flow

```
1. User → POST /v1/chat/completions
   ↓
2. API Handler receives request
   ↓
3. Convert messages to internal format
   ↓
4. Session Manager gets/creates session
   ↓
5. Memory Retriever builds context
   ↓
6. LLM Client sends request
   ↓
7. If tool calls:
   - Tool Registry executes
   - Results added to context
   - Loop back to step 6
   ↓
8. Response streamed to user
   ↓
9. Session updated with messages
   ↓
10. Memory references recorded
```

### Session Review Flow

```
1. Session ends (timeout or close)
   ↓
2. Session Review Service triggered
   ↓
3. For each conversation turn:
   - Generate training example
   - Evaluate quality
   - Store if above threshold
   ↓
4. Identify memories to evict
   ↓
5. Identify topics for learning
   ↓
6. Update session status
```

### Eviction Flow

```
1. TTL expires for memories
   ↓
2. Eviction Service evaluates each:
   - Check memory type
   - Check quality score
   - Check access count
   ↓
3. Decision made:
   - Keep (high quality, frequently accessed)
   - Evict to training (concepts, patterns)
   - Delete (facts, low quality)
   ↓
4. Execute eviction
   ↓
5. Generate training examples
   ↓
6. Update statistics
```

### Training Flow

```
1. Cron trigger (2 AM daily)
   ↓
2. Collect training examples
   - From session reviews
   - From evicted memories
   ↓
3. Format for GRPO
   - Prompt/completion pairs
   - Reward labels
   ↓
4. Load base model (LoRA)
   ↓
5. Training loop:
   - Sample batch
   - Compute rewards
   - Update policy
   ↓
6. Evaluate on holdout set
   ↓
7. Save new model version
   ↓
8. Update model registry
   ↓
9. (Optional) Hot-swap to new model
```

## API Reference

### Chat Completions

```bash
POST /v1/chat/completions

Request:
{
  "model": "agent",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": false
}

Response:
{
  "id": "chatcmpl-xxx",
  "choices": [{
    "message": {
      "role": "assistant",
      "content": "Hello! How can I help?"
    }
  }]
}
```

### Memory Query

```bash
POST /memories/query

Request:
{
  "query": "What did I learn about Rust?",
  "namespace": "retrieval",
  "limit": 10
}

Response:
{
  "results": [{
    "memory": {
      "id": "xxx",
      "content": "Rust uses ownership...",
      "tags": ["rust", "concept"]
    },
    "score": 0.85
  }]
}
```

### Training Trigger

```bash
POST /training/trigger

Response:
{
  "job_id": "xxx",
  "status": "started"
}
```

## Configuration

### Memory Configuration

```toml
[memory]
storage_path = ".agent/memory"
retrieval_ttl_hours = 24
default_namespace = "retrieval"
min_similarity = 0.6
query_limit = 10
```

### Training Configuration

```toml
[training]
enabled = true
schedule = "0 2 * * *"  # 2 AM daily
model = "qwen3:8b"
output_path = ".agent/models"
batch_size = 4
steps = 500
learning_rate = 5e-6
lora_rank = 16
```

## Security Considerations

### Bash Tool
- Command allowlist/denylist
- Working directory restrictions
- Timeout enforcement
- Output size limits

### File Tools
- Path restrictions
- Directory traversal prevention
- Permission checks

### Memory Access
- Namespace isolation
- Access logging
- Data encryption (future)

## Performance Optimizations

### Memory System
- Embedding cache (LRU)
- Batch embedding requests
- Async I/O for storage

### LLM Client
- Connection pooling
- Request timeout handling
- Streaming response handling

### Training
- LoRA for efficient fine-tuning
- Gradient accumulation
- Mixed precision (FP16/BF16)

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

## References

### Technologies Used
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [Tantivy](https://github.com/quickwit-oss/tantivy) - Vector search
- [SQLite](https://www.sqlite.org/) - Database
- [Candle](https://github.com/huggingface/candle) - ML framework
- [Tokio](https://tokio.rs/) - Async runtime

### Inspired By
- [Mule](https://github.com/mule-ai/mule) - Agent workflow platform
- [mem](https://github.com/jbutlerdev/mem) - Semantic memory CLI
- [search](https://github.com/mule-ai/search) - Search CLI
- [pi](https://github.com/mariozechner/pi) - Agent runtime
