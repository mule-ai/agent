# AGI Agent

A Rust-based AGI agent system combining memory management, session tracking, reinforcement learning, and multi-agent collaboration.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Python CLI                                     │
│                     (./agi chat)                                      │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Agent API (port 8080)                           │
│                   Rust HTTP Server (Axum)                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐   │
│  │  Session    │  │  Memory     │  │   LLM Client              │   │
│  │  Manager    │  │  Store      │  │   → llama.cpp            │   │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   llama.cpp Server (port 8081)                        │
│                    Qwen3.5-4B-Q8_0 Model                             │
└─────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Start llama.cpp Server

```bash
sudo systemctl start llama-qwen
sudo systemctl status llama-qwen
```

### 2. Build and Start Agent

```bash
./build.sh --bin agent
/tmp/target/release/agent
```

### 3. Chat with the Agent

```bash
./agi chat
```

## Services

### llama-qwen (Systemd)

LLM inference server serving Qwen3.5-4B-Q8_0 via llama.cpp.

```bash
sudo systemctl start llama-qwen    # Start
sudo systemctl stop llama-qwen    # Stop
sudo systemctl status llama-qwen   # Check status
```

### Agent API (Manual)

The Rust Agent handles session management, memory storage, and calls llama.cpp for LLM inference.

```bash
./build.sh --bin agent
/tmp/target/release/agent
```

## CLI Commands

```bash
./agi chat              # Interactive chat
./agi status            # Check memory/training status
./agi train             # Trigger RL training
./agi models            # List available models
```

## Python CLI

The Python CLI (`./agi` or `python3 cli.py`) is a simple client that calls the Agent API.

**Features:**
- Auto-detects Agent availability
- Falls back to direct llama.cpp if Agent not running
- Clean terminal UI with streaming responses

## Configuration

Edit `agent.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080

[model]
base_url = "http://10.10.199.146:8081"  # llama.cpp endpoint
name = "qwen3.5-4b"
embedding_model = "nomic-embed-text"
embedding_dim = 768
max_tokens = 4096
temperature = 0.7

[memory]
storage_path = "/home/administrator/.agi/memory"
retrieval_ttl_hours = 24
default_namespace = "retrieval"
min_similarity = 0.6
query_limit = 10

[training]
enabled = true
schedule = "0 2 * * *"
model = "qwen3.5-4b"
output_path = ".agent/models"
epochs = 3
batch_size = 4
learning_rate = 1e-4
lora_rank = 16

[search]
instance = "https://search.butler.ooo"
timeout = 30

[summarization]
provider = "openai"
api_key = ""
model = "gpt-4o-mini"

[online_learning]
batch_size = 16
max_buffer_size = 1000
replay_ratio = 0.3
learning_rate = 1e-5
min_buffer_for_training = 50
adaptive_learning_rate = true
update_interval_seconds = 300

[curiosity]
enabled = true
max_gaps = 50
curiosity_threshold = 0.5
exploration_depth = 2
```

## API Endpoints

### Chat Completions

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [
      {"role": "system", "content": "You are helpful."},
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

### Multi-modal Content

The agent supports multi-modal content including images and audio. Messages can include a content array:

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [
      {
        "role": "user",
        "content": [
          {"type": "text", "text": "What is in this image?"},
          {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
        ]
      }
    ]
  }'
```

### Memory

```bash
# List memories
curl http://localhost:8080/memories

# Query memories with semantic search
curl -X POST http://localhost:8080/memories/query \
  -H "Content-Type: application/json" \
  -d '{"query": "What did I learn?", "limit": 10, "namespace": "retrieval"}'

# Store memory
curl -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "Important fact", "tags": ["fact"], "memory_type": "fact"}'

# Delete memory
curl -X DELETE http://localhost:8080/memories/{id}
```

### Training

Training examples are automatically generated from sessions, search learning, and curiosity-driven exploration. They are persisted to `~/.agi/training/examples.jsonl` and can be exported for external training.

```bash
# Trigger training
curl -X POST http://localhost:8080/training/trigger

# Check status
curl http://localhost:8080/training/status

# Batch training endpoints
curl http://localhost:8080/training/batch/status
curl -X POST http://localhost:8080/training/batch/collect
curl -X POST http://localhost:8080/training/batch/add \
  -H "Content-Type: application/json" \
  -d '{"prompt": "What is Rust?", "completion": "Rust is a systems programming language."}'
curl http://localhost:8080/training/batch/stats
curl -X POST http://localhost:8080/training/batch/run
curl http://localhost:8080/training/batch/export
curl -X POST http://localhost:8080/training/batch/clear
curl -X POST http://localhost:8080/training/batch/filter \
  -H "Content-Type: application/json" \
  -d '{"threshold": 0.7}'

# Model registry
curl http://localhost:8080/training/models/list
curl -X POST http://localhost:8080/training/models/current \
  -H "Content-Type: application/json" \
  -d '{"model_id": "qwen3:8b-v20260329120000"}'
```

### Scheduler

```bash
# Get scheduler statistics
curl http://localhost:8080/scheduler/stats

# Manually trigger batch training
curl -X POST http://localhost:8080/scheduler/trigger
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

### Model Hot-Swap

```bash
# Get current model status
curl http://localhost:8080/model/status

# List available models on endpoint
curl http://localhost:8080/model/available

# Validate a model configuration
curl -X POST http://localhost:8080/model/validate \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen3:8b", "base_url": "http://localhost:11434"}'

# Hot-swap to a new model (no restart needed)
curl -X POST http://localhost:8080/model/update \
  -H "Content-Type: application/json" \
  -d '{"model": "llama3:70b"}'
```

### Sessions

Sessions automatically trigger review when ended, which extracts facts, concepts, and generates training examples.

```bash
# List all sessions
curl http://localhost:8080/sessions

# Create a new session
curl -X POST http://localhost:8080/sessions \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123"}'

# Get a specific session
curl http://localhost:8080/sessions/{id}

# End a session (triggers session review automatically)
curl -X POST http://localhost:8080/sessions/{id}/end

# Delete a session
curl -X DELETE http://localhost:8080/sessions/{id}
```

### Curiosity Engine

The curiosity engine enables autonomous exploration of topics the agent doesn't understand well.

```bash
# Get curiosity statistics
curl http://localhost:8080/curiosity/stats

# Detect knowledge gaps in a conversation
curl -X POST http://localhost:8080/curiosity/detect \
  -H "Content-Type: application/json" \
  -d '{"messages": [{"role": "user", "content": "Explain neural networks"}]}'

# List all detected gaps
curl http://localhost:8080/curiosity/gaps

# Get pending gaps needing exploration
curl http://localhost:8080/curiosity/gaps/pending

# Explore a specific gap
curl -X POST http://localhost:8080/curiosity/explore \
  -H "Content-Type: application/json" \
  -d '{"gap_id": "uuid-here"}'

# Process exploration queue
curl -X POST http://localhost:8080/curiosity/process \
  -H "Content-Type: application/json" \
  -d '{"max_explorations": 5}'

# Dismiss a gap
curl -X POST http://localhost:8080/curiosity/dismiss \
  -H "Content-Type: application/json" \
  -d '{"gap_id": "uuid-here"}'
```

### Online Learning

Continuous reinforcement learning from interactions using experience replay.

```bash
# Get learning statistics
curl http://localhost:8080/learning/stats

# Get replay buffer stats
curl http://localhost:8080/learning/buffer

# Add a training example
curl -X POST http://localhost:8080/learning/example \
  -H "Content-Type: application/json" \
  -d '{"prompt": "What is Rust?", "completion": "Rust is a systems programming language..."}'

# Add all session experiences
curl -X POST http://localhost:8080/learning/session \
  -H "Content-Type: application/json" \
  -d '{"messages": [{"role": "user", "content": "Hello"}, {"role": "assistant", "content": "Hi!"}]}'

# Perform learning update
curl -X POST http://localhost:8080/learning/learn \
  -H "Content-Type: application/json" \
  -d '{"batch_size": 16}'

# Get learned concepts with strengths
curl http://localhost:8080/learning/concepts

# Prune trained examples from buffer
curl -X POST http://localhost:8080/learning/prune
```

### Self-Improvement

The self-improvement engine analyzes code patterns and generates improvements to the agent.

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

# Reject an improvement
curl -X POST http://localhost:8080/self-improve/reject \
  -H "Content-Type: application/json" \
  -d '{"improvement_id": "uuid-here"}'

# Rollback an applied improvement
curl -X POST http://localhost:8080/self-improve/rollback \
  -H "Content-Type: application/json" \
  -d '{"improvement_id": "uuid-here"}'

# Get current system prompt
curl http://localhost:8080/self-improve/prompt

# Update system prompt
curl -X POST http://localhost:8080/self-improve/prompt \
  -H "Content-Type: application/json" \
  -d '{"prompt": "You are a helpful coding assistant..."}'
```

### Theory of Mind

The theory of mind engine models user mental state for personalized responses.

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
curl http://localhost:8080/tom/user?user_id=user-123

# Get all user models
curl http://localhost:8080/tom/users

# Analyze user for response recommendations
curl -X POST http://localhost:8080/tom/analyze \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123"}'

# Get conversation history
curl http://localhost:8080/tom/history?user_id=user-123

# Clear user model
curl -X POST http://localhost:8080/tom/clear \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123"}'

# Update trust level
curl -X POST http://localhost:8080/tom/trust \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123", "delta": 0.1}'

# Satisfy an intention
curl -X POST http://localhost:8080/tom/intention \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123", "intention_type": "Learn"}'
```

### External Knowledge

Query Wikipedia, ArXiv, and general web content.

```bash
# Search knowledge base
curl -X POST http://localhost:8080/knowledge/search \
  -H "Content-Type: application/json" \
  -d '{"query": "rust programming", "limit": 5, "source": "wikipedia"}'

# Get Wikipedia article
curl "http://localhost:8080/knowledge/wikipedia/Rust_(programming_language)"

# Get ArXiv paper
curl "http://localhost:8080/knowledge/arxiv/2303.08774"

# Fetch web page
curl -X POST http://localhost:8080/knowledge/fetch \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com"}'

# Get knowledge sources status
curl http://localhost:8080/knowledge/sources
```

## Memory System

### Namespaces

- **retrieval**: Short-term memory for current context (TTL-based eviction after 24 hours)
- **training**: Long-term memory for RL training data (persistent, accumulates over time)
- **team**: Shared memory for multi-agent collaboration

### Memory Types

| Type | Description |
|------|-------------|
| `Fact` | Specific facts learned (evaluated for quality before eviction) |
| `Concept` | Generalizations and patterns (moved to training namespace) |
| `Conversation` | Conversation transcripts |
| `ToolResult` | Results from tool execution |

### Eviction Policy

- TTL-based expiration (24 hours default for retrieval namespace)
- High-quality concepts move to training namespace for persistence
- Facts are evaluated for quality before eviction
- Low-quality memories are deleted
- Training namespace never evicts (accumulates over time)

## Tool System

### Available Tools

| Tool | Purpose |
|------|---------|
| `search` | Web search via SearXNG |
| `fetch` | Fetch webpage content |
| `fetch_image` | Fetch images from URLs or local files |
| `bash` | Execute shell commands |
| `read` | Read file contents |
| `write` | Write file contents |

Tools are exposed via OpenAI function calling schema and results are automatically added to memory.

## Background Services

| Service | Description |
|---------|-------------|
| `SessionReview` | Analyzes conversations on session end, extracts facts/concepts, generates training examples |
| `MemoryEviction` | Manages memory lifecycle, applies eviction policies |
| `SearchLearning` | Researches topics using SearXNG, generates training examples |
| `BatchTraining` | GRPO training pipeline with model registry, persists examples to disk |
| `OnlineLearning` | Continuous RL from tool interactions using experience replay |
| `Curiosity` | Autonomous exploration of knowledge gaps, wired to batch training |
| `SelfImprove` | Analyzes code patterns, generates improvements |
| `TheoryOfMind` | Models user mental state for personalized responses |
| `Scheduler` | Cron-based scheduler for automated background tasks (batch training, eviction, review) |

### Scheduler

The scheduler service runs automated background tasks based on configurable cron schedules:

```bash
# Get scheduler statistics
curl http://localhost:8080/scheduler/stats

# Manually trigger batch training
curl -X POST http://localhost:8080/scheduler/trigger
```

**Default Schedule:**
- Batch Training: 2 AM daily (`0 2 * * *`)
- Memory Eviction: Midnight daily (`0 0 * * *`)
- Session Review: Every 6 hours (`0 */6 * * *`)

**Configuration (`agent.toml`):**
```toml
[scheduler]
enabled = true
batch_training_enabled = true
batch_training_schedule = "0 2 * * *"
memory_eviction_enabled = true
memory_eviction_schedule = "0 0 * * *"
```

## Multi-Agent Teams

The agent supports multi-agent collaboration with shared memory:

```bash
# Create a team with specialized roles
curl -X POST http://localhost:8080/team/create \
  -H "Content-Type: application/json" \
  -d '{
    "roles": ["assistant", "coder", "researcher", "writer", "analyst"]
  }'
```

### Agent Roles

- **Assistant**: General help and conversation
- **Coder**: Code-related tasks (detected by "code", "function", "debug" keywords)
- **Researcher**: Research tasks (detected by "research", "find", "search" keywords)
- **Writer**: Writing and editing tasks
- **Analyst**: Analysis tasks

## Training Pipeline

### GRPO Training

Group Relative Policy Optimization with reward functions:

```rust
// Format reward for structured output
format_reward(completion) {
    score += 1.0 if contains("<REASONING>") && contains("</REASONING>")
    score += 1.0 if contains("<SOLUTION>") && contains("</SOLUTION>")
}

// Helpfulness reward for quality responses
helpfulness_reward(completion, reference) {
    similarity(completion, reference)
}
```

### Training Flow

Training examples are automatically generated from multiple sources and persisted to disk:

```
┌─────────────────┐
│  User Sessions  │
│  (Conversations)│
└────────┬────────┘
         │ Session End
         ▼
┌─────────────────┐
│ Session Review  │──────► Facts & Concepts extracted
│ (LLM-enhanced)  │       Training examples generated
└────────┬────────┘       Mem: ~/.agi/training/examples.jsonl
         │
         ▼
┌─────────────────┐
│ Search Learning │──────► Research results
│ (SearXNG)      │       Training examples generated
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Curiosity Engine│──────► Knowledge gaps explored
│ (Autonomous)    │       Training examples generated
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────────────┐
│           BatchTrainingService                  │
│  Examples persisted: ~/.agi/training/examples.jsonl
└────────┬────────────────────────────────────────┘
         │
         │ Cron (default: 2 AM)
         ▼
┌─────────────────┐
│ Scheduled GRPO │
│ Training       │──────► LoRA Adapter generated
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Model Registry  │──────► Hot-swap ready
└─────────────────┘
```

### Online Learning

Continuous learning using prioritized experience replay:

- Experience replay buffer (max 1000 experiences)
- Priority = (reward * 0.6) + (quality * 0.4) + novelty_bonus
- Adaptive learning rate based on recent performance
- 30% of each batch from replay, 70% fresh examples

## Project Structure

```
agent/
├── src/
│   ├── main.rs              # Entry point
│   ├── agent/               # Agent core
│   │   ├── mod.rs           # Agent struct with memory integration
│   │   ├── session.rs       # Session management
│   │   ├── session_store.rs  # SQLite session storage
│   │   ├── reasoning.rs     # Reasoning engine (LLM-powered)
│   │   ├── llm.rs           # LLM client (→ llama.cpp)
│   │   └── team.rs          # Multi-agent team system
│   ├── api/                 # HTTP handlers
│   │   ├── chat.rs          # Chat completions with multi-modal
│   │   ├── memory.rs        # Memory endpoints
│   │   ├── training.rs      # Training endpoints
│   │   ├── models.rs        # Model hot-swap, concepts
│   │   ├── sessions.rs      # Session management
│   │   ├── services.rs      # Curiosity, learning, self-improve, ToM
│   │   └── knowledge.rs     # External knowledge base
│   ├── memory/              # Memory system
│   │   ├── store.rs         # SQLite + Tantivy vector storage
│   │   ├── embedding.rs     # Embedding client (Ollama API)
│   │   ├── retrieval.rs     # Memory retriever
│   │   └── eviction.rs      # Eviction policies
│   ├── services/            # Background services
│   │   ├── session_review.rs     # Session analysis
│   │   ├── memory_eviction.rs    # Memory lifecycle
│   │   ├── search_learning.rs    # Topic research
│   │   ├── batch_training.rs     # Training pipeline
│   │   ├── online_learning.rs    # Continuous RL
│   │   ├── curiosity.rs          # Curiosity-driven
│   │   ├── self_improve.rs       # Self-improvement
│   │   └── theory_of_mind.rs     # User modeling
│   ├── training/            # Training pipeline (GRPO)
│   ├── knowledge/           # External knowledge
│   │   ├── wikipedia.rs     # Wikipedia API
│   │   ├── arxiv.rs         # ArXiv API
│   │   └── fetch.rs         # Web fetcher
│   ├── models/              # Data models
│   └── config/              # Configuration
├── cli.py                   # Python CLI client
├── agi                      # CLI wrapper script
├── agent.toml              # Configuration
├── build.sh                # Build script
└── TRAINING.md             # Training documentation
```

## Build

The project uses SMB-mounted storage which doesn't support Rust build scripts. Use the provided build script:

```bash
./build.sh --bin agent        # Build agent
./build.sh --bin cli         # Build CLI
```

Output goes to `/tmp/target/release/`

## Development

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## Dependencies

| Component | Technology |
|-----------|------------|
| HTTP Server | Axum |
| Memory | SQLite + Tantivy (vector search) |
| LLM Client | llama.cpp (via HTTP/OpenAI-compatible) |
| Async Runtime | Tokio |
| Serialization | Serde |
| Multi-modal | Base64 image/audio support |
| Sessions | SQLite persistence |
| External Knowledge | Wikipedia, ArXiv APIs |
| Embeddings | Ollama API (nomic-embed-text) |

## License

MIT
