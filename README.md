# AGI Agent

A Rust-based AGI agent with memory management, session tracking, and RL training capabilities.

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
│  │  Manager    │  │  Store     │  │   → llama.cpp            │   │
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
sudo systemctl stop llama-qwen       # Stop
sudo systemctl status llama-qwen    # Check status
```

**Configuration:** `/home/administrator/qwen35-4b.sh`

### Agent API (Manual)

The Rust Agent handles session management, memory storage, and calls llama.cpp for LLM inference.

```bash
./build.sh --bin agent
/tmp/target/release/agent
```

**Configuration:** `agent.toml`

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

[memory]
storage_path = "/home/administrator/.agi/memory"

[training]
enabled = true
model = "qwen3.5-4b"
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

### Memory

```bash
# List memories
curl http://localhost:8080/memories

# Query memories
curl -X POST http://localhost:8080/memories/query \
  -H "Content-Type: application/json" \
  -d '{"query": "What did I learn?"}'

# Store memory
curl -X POST http://localhost:8080/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "Important fact", "tags": ["fact"]}'
```

### Training

```bash
# Trigger training
curl -X POST http://localhost:8080/training/trigger

# Check status
curl http://localhost:8080/training/status

# List models
curl http://localhost:8080/training/models
```

## Memory System

### Namespaces

- **retrieval**: Short-term memory for current context
- **training**: Long-term memory for RL training data

### Memory Types

| Type | Description |
|------|-------------|
| `Fact` | Specific facts learned |
| `Concept` | Generalizations and patterns |
| `Conversation` | Conversation transcripts |
| `ToolResult` | Results from tool execution |

### Eviction Policy

- TTL-based expiration (24 hours default)
- Concepts move to training namespace
- Facts are evaluated for quality
- Low-quality memories are deleted

## Project Structure

```
agent/
├── src/
│   ├── main.rs              # Entry point
│   ├── agent/               # Agent core
│   │   ├── mod.rs           # Agent struct
│   │   ├── session.rs       # Session management
│   │   ├── reasoning.rs     # Reasoning engine
│   │   └── llm.rs          # LLM client (→ llama.cpp)
│   ├── api/                 # HTTP handlers
│   │   ├── chat.rs         # Chat completions
│   │   ├── memory.rs       # Memory endpoints
│   │   └── training.rs      # Training endpoints
│   ├── memory/              # Memory system
│   │   ├── store.rs        # SQLite + Tantivy
│   │   ├── embedding.rs     # Embedding client
│   │   └── eviction.rs     # Eviction policies
│   ├── models/              # Data models
│   └── config/              # Configuration
├── cli.py                    # Python CLI
├── agi                       # CLI wrapper script
├── agent.toml               # Configuration
├── build.sh                 # Build script
└── CLI.md                   # CLI documentation
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
|----------|------------|
| HTTP Server | Axum |
| Memory | SQLite + Tantivy |
| LLM Client | llama.cpp (via HTTP) |
| Async Runtime | Tokio |
| Serialization | Serde |

## License

MIT
