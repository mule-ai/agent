# AGI Agent

A Rust-native AGI agent with extensive memory management and reinforcement learning capabilities.

## Overview

This project implements an intelligent agent that learns from interactions through:

- **Extensive Memory System**: Short-term retrieval memory + long-term training memory
- **Background Learning**: Automatic session review and topic research
- **RL Training Pipeline**: Overnight training using accumulated experiences
- **OpenAI-Compatible API**: Drop-in replacement for existing applications

## Quick Start

```bash
# Clone and build
cargo build --release

# Configure
cp agent.toml.example agent.toml
# Edit agent.toml with your settings

# Run
cargo run --release
```

```bash
# Chat with the agent
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     User Interface Layer                          │
│              OpenAI-Compatible Chat API (Axum)                    │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Agent Core                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │   Session    │  │  Reasoning  │  │   Tool Execution     │  │
│  │  Manager     │  │   Engine    │  │  (Search, Bash...)   │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Memory System (SQLite + Tantivy)                │
│  ┌────────────────────┐          ┌────────────────────────────┐ │
│  │  Retrieval Memory  │    TTL   │    Training Memory         │ │
│  │  (Short-term)     │ ───────► │    (Long-term)            │ │
│  └────────────────────┘  evict  └────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Background Services                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐   │
│  │ Session Review  │  │ Search Learning │  │ Eviction      │   │
│  │ (Training Data) │  │ (Topic Research)│  │ (RL Policies) │   │
│  └─────────────────┘  └─────────────────┘  └────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                   RL Training Pipeline                             │
│         GRPO / LoRA Fine-tuning (Candle ML Framework)           │
└─────────────────────────────────────────────────────────────────┘
```

## Features

### Memory System

| Feature | Description |
|---------|-------------|
| **Semantic Search** | Tantivy vector search with embeddings |
| **Namespace Support** | Separate retrieval and training namespaces |
| **Automatic Eviction** | TTL-based expiration with smart policies |
| **Quality Scoring** | Memory quality assessment for training |
| **Embedding Cache** | LRU cache for fast retrieval |

### Background Services

| Service | Trigger | Action |
|---------|---------|--------|
| **Session Review** | On session end | Generate training examples |
| **Search Learning** | Knowledge gaps detected | Research topics via SearXNG |
| **Memory Eviction** | TTL expiration | Move to training or delete |
| **RL Training** | Cron schedule (2 AM) | Fine-tune model with GRPO |

### Tool System

- **Search**: Web search via SearXNG
- **Fetch**: Webpage content extraction
- **Bash**: Secure command execution
- **File Tools**: Read/write with path restrictions

### Training Pipeline

- **Algorithm**: Group Relative Policy Optimization (GRPO)
- **Efficiency**: LoRA fine-tuning (rank 16)
- **Rewards**: Format and correctness scoring
- **Model Registry**: Version tracking with hot-swap

## Installation

### Prerequisites

- Rust 1.75+ (with Cargo)
- SQLite
- Ollama (for local LLM) or OpenAI-compatible API
- SearXNG instance (for web search)

### Build

```bash
# Build main agent
cargo build --release

# Build training binary
cargo build --release --bin training

# Build memory CLI
cargo build --release --bin mem-cli
```

### Configuration

Edit `agent.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080

[model]
base_url = "http://localhost:11434"
name = "qwen3:8b"
embedding_model = "nomic-embed-text"

[memory]
storage_path = ".agent/memory"
retrieval_ttl_hours = 24

[training]
enabled = true
schedule = "0 2 * * *"  # 2 AM daily
steps = 500
```

## Usage

### Start the Agent

```bash
cargo run --release
```

### API Examples

```bash
# Chat completion
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [
      {"role": "user", "content": "Explain quantum computing"}
    ],
    "stream": false
  }'

# List models
curl http://localhost:8080/v1/models

# Query memories
curl -X POST http://localhost:8080/memories/query \
  -H "Content-Type: application/json" \
  -d '{"query": "What did I learn about Rust?"}'

# Store a memory
curl -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Rust uses ownership for memory safety",
    "tags": ["rust", "programming"],
    "memory_type": "concept"
  }'

# Trigger training
curl -X POST http://localhost:8080/training/trigger

# Check training status
curl http://localhost:8080/training/status
```

### Memory CLI

```bash
# Store a memory
./target/release/mem-cli store \
  --content "Important fact" \
  --tags "fact"

# Query memories
./target/release/mem-cli query --query "user preferences"

# List memories
./target/release/mem-cli list --limit 100

# Show statistics
./target/release/mem-cli stats
```

## How It Works

### Conversation Flow

1. **Request Received**: User sends message via `/v1/chat/completions`
2. **Session Management**: Get or create conversation session
3. **Memory Retrieval**: Find relevant memories for context
4. **Response Generation**: LLM generates response (with optional tool use)
5. **Response Streamed**: Real-time response to user
6. **Session Updated**: Messages and memories recorded

### Memory Lifecycle

```
┌─────────────┐
│  Created    │  New memory from conversation
└──────┬──────┘
       │
       ▼ (TTL: 24 hours)
┌─────────────┐
│  Eviction   │  Evaluate for training or deletion
│  Decision   │
└──────┬──────┘
       │
       ├──────► Concept/Learned → Move to "training" namespace
       │
       ├──────► Frequently accessed → Keep in "retrieval"
       │
       └──────► Fact/Low quality → Delete
```

### Training Pipeline

1. **Collect**: Gather training examples from sessions
2. **Format**: Prepare prompts/completions with rewards
3. **Train**: Run GRPO with LoRA adapters
4. **Evaluate**: Check quality on holdout set
5. **Deploy**: Hot-swap to new model version

## Project Structure

```
agent/
├── src/
│   ├── main.rs                    # Entry point
│   ├── agent/                     # Agent core
│   │   ├── mod.rs                # Agent struct
│   │   ├── session.rs            # Session management
│   │   ├── reasoning.rs          # Reasoning engine
│   │   └── llm.rs               # LLM client
│   ├── api/                      # HTTP handlers
│   │   ├── mod.rs               # Router setup
│   │   ├── chat.rs              # Chat completions
│   │   ├── memory.rs            # Memory endpoints
│   │   └── training.rs           # Training endpoints
│   ├── memory/                   # Memory system
│   │   ├── mod.rs               # Config types
│   │   ├── store.rs             # SQLite + Tantivy
│   │   ├── embedding.rs          # Embedding client
│   │   ├── retrieval.rs          # Memory retriever
│   │   └── eviction.rs           # Eviction policies
│   ├── tools/                    # Tool system
│   │   ├── mod.rs               # Tool trait
│   │   ├── search.rs             # Search tool
│   │   ├── bash.rs               # Bash tool
│   │   ├── files.rs              # File tools
│   │   └── registry.rs           # Tool registry
│   ├── services/                 # Background services
│   │   ├── mod.rs               # Service trait
│   │   ├── session_review.rs     # Training data generation
│   │   ├── search_learning.rs    # Topic research
│   │   └── memory_eviction.rs    # Memory management
│   ├── config/                   # Configuration
│   ├── models/                   # Data models
│   └── training/                  # Training pipeline
├── training/
│   └── main.rs                   # Training binary
├── mem_cli/
│   └── main.rs                   # Memory CLI
├── agent.toml                     # Configuration
├── Cargo.toml
└── README.md
```

## Development

### Run Tests

```bash
cargo test
cargo test --package memory
cargo test --package agent
```

### Code Quality

```bash
cargo fmt
cargo clippy
```

### Dependencies

| Crate | Purpose |
|-------|---------|
| `axum` | HTTP server + WebSocket |
| `tantivy` | Vector search |
| `rusqlite` | SQLite database |
| `candle` | ML framework |
| `tokio` | Async runtime |
| `reqwest` | HTTP client |

## License

MIT License
