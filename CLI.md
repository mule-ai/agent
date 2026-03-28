# AGI Agent CLI

Simple Python CLI that calls the Rust Agent API. The Agent handles all logic (LLM calls, memory, sessions, training).

## Architecture

```
CLI (Python) → Agent API (Rust, port 8080) → llama.cpp (port 8081)
                        ↓
                  Memory/Sessions
                        ↓
                  Training Pipeline
```

## Quick Start

```bash
# Make sure llama.cpp is running
sudo systemctl status llama-qwen

# Chat (CLI auto-detects Agent availability)
./agi chat

# If Agent isn't running, CLI falls back to direct llama.cpp
```

## Commands

```bash
./agi chat              # Interactive chat
./agi status            # Check Agent status
./agi train            # Trigger RL training (via Agent)
./agi models           # List available models
```

## How It Works

### When Agent is Running

```
CLI → Agent API → llama.cpp
            ↓
      Memory (SQLite/Tantivy)
            ↓
      Sessions
            ↓
      Training Pipeline
```

### When Agent is NOT Running

```
CLI → llama.cpp (direct)
            ↓
   No memory/sessions (just chat works)
```

The CLI detects Agent availability automatically and shows:
- **Agent: Connected ✓** - Full functionality
- **Agent: Not running** - Chat works, memory disabled

## Prerequisites

### llama.cpp server
```bash
sudo systemctl status llama-qwen
sudo systemctl start llama-qwen
```

### Agent (optional - enables memory/sessions)
```bash
# Agent binary needs to be built and run
# When running, it connects to llama.cpp
cargo run --release
```

## Configuration

Default endpoints in `cli.py`:
```python
AGENT_URL = "http://localhost:8080"  # Rust Agent
LLAMA_URL = "http://10.10.199.146:8081"  # llama.cpp
```

## Services

| Service | Port | Status |
|---------|------|--------|
| llama-qwen | 8081 | Qwen3.5-4B-Q8_0 |
| agent | 8080 | Optional (enables memory) |

## Troubleshooting

### "Agent: Not running"
This is fine - chat still works. To enable memory/sessions, start the Rust Agent.

### Connection refused
```bash
# Check llama.cpp
sudo systemctl status llama-qwen

# Or check Agent
curl http://localhost:8080/health
```
