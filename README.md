# AGI Agent

A Rust-based AGI agent system combining memory management, session tracking, reinforcement learning, and multi-agent collaboration.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Python CLI (./agi)                                   │
│                    Simple client - calls Agent API                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Agent API (port 8080)                                │
│                      Rust HTTP Server (Axum)                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ Session Manager │  │  Memory Store   │  │      LLM Client            │ │
│  │ (State)        │  │  (SQLite)       │  │      → llama.cpp           │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ Reasoning       │  │ RAG Retrieval   │  │    Tool Registry           │ │
│  │ Engine          │  │ (Text Search)  │  │    (search, bash, etc)    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                   llama.cpp Server (port 8081)                               │
│                     Qwen3.5-4B-Q8_0 Model (symlink)                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                    ┌─────────────────┴─────────────────┐
                    ▼                                   ▼
┌───────────────────────────────┐     ┌───────────────────────────────────────┐
│ Qwen3-Embedding-0.6B (8083)  │     │ Qwen3-Reranker-0.6B (8084)            │
│ Vector embeddings for memory   │     │ Optional reranking for search         │
└───────────────────────────────┘     └───────────────────────────────────────┘
```

## Services (Systemd)

| Service | Port | Description |
|---------|------|-------------|
| `llama-qwen` | 8081 | Qwen3.5-4B inference (symlink: `~/qwen35-trained-latest.gguf`) |
| `llama-embedding` | 8083 | Qwen3-Embedding-0.6B for vector search |
| `llama-rerank` | 8084 | Qwen3-Reranker-0.6B for result reranking |
| `agi-agent` | 8080 | Main agent API |

## Quick Start

### 1. Start All Services

```bash
# Start all services
sudo systemctl start llama-qwen
sudo systemctl start llama-embedding
sudo systemctl start llama-rerank
sudo systemctl start agi-agent

# Check status
sudo systemctl status llama-qwen agi-agent
```

### 2. Chat with the Agent

```bash
./agi chat
# or
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "agent", "messages": [{"role": "user", "content": "What is my name?"}]}'
```

## CLI Commands

```bash
./agi chat              # Interactive chat
./agi status            # Check Agent status and memory counts
./agi train             # Trigger RL training (collect → train → deploy)
./agi data [query]      # Query training data from SQLite
./agi research [topic]  # Trigger search learning for a topic
./agi models            # List available trained models
```

## Configuration

Edit `agent.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080

[model]
base_url = "http://10.10.199.146:8081"  # llama.cpp endpoint
name = "qwen3.5-4b"
embedding_model = "qwen3-embedding"           # Qwen3 embedding model
embedding_base_url = "http://127.0.0.1:8083" # Embedding server
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
schedule = "0 2 * * *"  # 2 AM daily
model = "qwen3.5-4b"
output_path = ".agent/models"
epochs = 3
batch_size = 4
learning_rate = 1e-4
lora_rank = 16

[search]
instance = "https://search.butler.ooo"  # SearXNG
timeout = 30

[online_learning]
batch_size = 16
max_buffer_size = 1000
replay_ratio = 0.3
learning_rate = 1e-5

[curiosity]
enabled = true
max_gaps = 50
curiosity_threshold = 0.5
exploration_depth = 2
```

## Memory System

### Architecture

The memory system uses a three-tier approach:

```
┌─────────────────────────────────────────────────────────────┐
│                    Retrieval Namespace                        │
│  Purpose: Short-term context for conversations              │
│  Storage: SQLite                                           │
│  TTL: 24 hours (configurable)                              │
│  Eviction: Move to training namespace                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ On TTL expiration
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Training Namespace                        │
│  Purpose: Long-term knowledge for RL training               │
│  Storage: SQLite (persistent)                               │
│  TTL: Never (accumulates over time)                         │
│  Usage: Training examples + RAG context                      │
└─────────────────────────────────────────────────────────────┘
```

### RAG (Retrieval-Augmented Generation)

RAG uses fast SQLite text search to retrieve relevant memories at query time:

```rust
// Query both namespaces for relevant memories
let memories = memory_store.search_by_text("training", "name", 5)?;
// Memories are injected into system prompt as context
```

**Features:**
- Fast text search (no embedding generation at query time)
- Searches both `retrieval` and `training` namespaces
- Filters stop words from query
- Returns most recent matching memories

### Memory Eviction

When retrieval memories expire (TTL):
- **Facts** → Move to training namespace (keeps knowledge for RL + RAG)
- **Concepts** → Already in training namespace
- **Conversations** → Deleted (no longer needed)

## Training Pipeline

### Flow

```
1. Session ends → Session review extracts facts/concepts
2. Memory eviction → Moves facts to training namespace  
3. Nightly training (cron) → Collects from training namespace
4. GRPO/LoRA training → Fine-tunes model
5. Export to GGUF → Merges adapters + exports
6. Deploy → Updates symlink + restarts llama-qwen.service
```

### Manual Training

```bash
./agi train
```

This:
1. Collects training data from `~/.agi/memory/memories.db`
2. Runs training with unsloth (Qwen3.5-4B)
3. Exports to GGUF format
4. Updates `~/qwen35-trained-latest.gguf` symlink
5. Restarts `llama-qwen.service`

### Training Data

Training data is stored in SQLite:
- Location: `~/.agi/memory/memories.db`
- Table: `memories`
- Namespace: `training`

Query directly:
```bash
./agi data jeremiah
```

## API Endpoints

### Chat Completions

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agent",
    "messages": [
      {"role": "user", "content": "What is my name?"}
    ]
  }'
```

### Memory

```bash
# List memories by namespace
curl "http://localhost:8080/memories?namespace=training&limit=10"

# Query memories with text search
curl -X POST http://localhost:8080/memories/query \
  -H "Content-Type: application/json" \
  -d '{"query": "jeremiah", "namespace": "training", "limit": 5}'

# Store memory
curl -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "My name is Jeremiah", "tags": ["fact"], "memory_type": "fact"}'
```

### Training

```bash
# Trigger training
curl -X POST http://localhost:8080/training/trigger

# Check status
curl http://localhost:8080/training/status

# Batch training
curl -X POST http://localhost:8080/training/batch/collect   # Collect from memory
curl -X POST http://localhost:8080/training/batch/run       # Run training
curl http://localhost:8080/training/batch/export            # Export JSONL
curl -X POST http://localhost:8080/training/batch/clear     # Clear examples
```

### Scheduler

```bash
curl http://localhost:8080/scheduler/stats
curl -X POST http://localhost:8080/scheduler/trigger
```

## Systemd Services

### llama-qwen.service
Serves the Qwen3.5-4B model via llama.cpp. Uses symlink `~/qwen35-trained-latest.gguf`.

### llama-embedding.service
Serves Qwen3-Embedding-0.6B for vector embeddings on port 8083.

### llama-rerank.service
Serves Qwen3-Reranker-0.6B for result reranking on port 8084.

### agi-agent.service
Main agent API on port 8080.

## Model Management

Models are stored in `.agent/models/`:
```
.agent/models/
├── qwen35-trained-1774719656-Q8_0.gguf  # Latest trained
├── qwen35-trained-1774718908-Q8_0.gguf  # Previous
└── ...
```

The symlink always points to the latest:
```bash
~/qwen35-trained-latest.gguf → .agent/models/qwen35-trained-XXX-Q8_0.gguf
```

After training, the symlink is updated and llama-qwen.service is restarted.

## Directory Structure

```
/data/jbutler/mule/agent/
├── src/                    # Rust source code
│   ├── agent/              # Core agent logic
│   ├── memory/             # Memory store (SQLite)
│   ├── tools/              # Tool implementations
│   ├── services/           # Background services
│   └── training/           # Training pipeline
├── cli.py                  # Python CLI client
├── agent.toml              # Configuration
├── training_script.py      # Training script (unsloth)
└── .agent/
    └── models/             # Trained model files

~/.agi/
├── memory/memories.db      # SQLite memory database
└── training/examples.jsonl # Training data export
```

## Troubleshooting

### Agent not responding
```bash
sudo systemctl status agi-agent
curl http://localhost:8080/health
```

### Model not responding
```bash
sudo systemctl status llama-qwen
curl http://10.10.199.146:8081/health
```

### RAG not finding memories
```bash
# Check training namespace
./agi data jeremiah

# Or query directly
curl -X POST http://localhost:8080/memories/query \
  -d '{"query": "name", "namespace": "training", "limit": 5}'
```

### Training stuck
```bash
# Check if training script is running
ps aux | grep training

# Check training examples count
./agi data --count
```
