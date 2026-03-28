# AGI Agent CLI

Simple Python CLI that calls the Rust Agent API. The Agent handles all logic (LLM calls, memory, sessions, training).

## Architecture

```
CLI (Python) → Agent API (Rust, port 8080) → llama.cpp (port 8081)
                           ↓
                     Memory (SQLite)
                           ↓
                     Training Pipeline
```

## Quick Start

```bash
# Start services
sudo systemctl start llama-qwen
sudo systemctl start agi-agent

# Chat (CLI auto-detects Agent availability)
./agi chat
```

## Commands

```bash
./agi chat              # Interactive chat
./agi status            # Check Agent status and memory counts
./agi train             # Trigger RL training (collect → train → deploy)
./agi data [query]      # Query training data from SQLite
./agi research [topic]  # Trigger search learning for a topic
./agi models            # List available trained models
```

### chat
Interactive chat with the agent. The agent uses RAG to retrieve memories and can access tools (search, bash, read, write).

```
$ ./agi chat
Agent: Connected ✓
Memory: 123 retrieval, 1193 training

You: What is my name?
Agent: Your name is Jeremiah.
```

### status
Check system status including memory counts and service availability.

```bash
$ ./agi status
Agent: Connected ✓
Memory:
  - retrieval: 123 memories
  - training: 1193 memories
  - total: 1316

Services:
  - llama-qwen: Running (port 8081)
  - llama-embedding: Running (port 8083)
  - agi-agent: Running (port 8080)

Training:
  - Last run: 2026-03-28 20:00
  - Examples: 451
  - Latest model: qwen35-trained-1774719656-Q8_0.gguf
```

### train
Trigger the full training pipeline:
1. Collect training data from memory database
2. Run GRPO/LoRA training with unsloth
3. Export to GGUF format
4. Update symlink `~/qwen35-trained-latest.gguf`
5. Restart `llama-qwen.service`

```bash
$ ./agi train
Starting training pipeline...
1. Collecting from memory database...
2. Running training (Qwen3.5-4B, 3 epochs)...
3. Exporting to GGUF...
4. Deploying to ~/qwen35-trained-latest.gguf
5. Restarting llama-qwen.service...
Done! Trained model deployed.
```

### data [query]
Query training data directly from SQLite database.

```bash
# Count all training examples
$ ./agi data --count
Training examples: 451

# Search for specific content
$ ./agi data jeremiah
ID: 1b27f15d-84c2-4d5f-b...
[training] my name is jeremiah and i am learning rust programming language...
Created: 2026-03-28

ID: b98fd542-14ab-4380-b...
[training] jeremiah told me his name is jeremiah...
Created: 2026-03-28
```

### research [topic]
Trigger search learning for a specific topic. The agent will:
1. Search for information on the topic
2. Store findings as memories
3. Add to training data

```bash
$ ./agi research "rust ownership model"
Researching: rust ownership model
1. Searching web...
2. Storing findings...
3. Adding to training data...
Done! Added 5 new training examples.
```

### models
List available trained models.

```bash
$ ./agi models
Available models:
  - qwen35-trained-1774719656-Q8_0.gguf (4.2GB) ← current
  - qwen35-trained-1774718908-Q8_0.gguf (4.2GB)
  - qwen35-trained-1774718000-Q8_0.gguf (4.2GB)
```

## How It Works

### When Agent is Running

```
CLI → Agent API → llama.cpp
            ↓
      Memory (SQLite text search)
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
| llama-embedding | 8083 | Qwen3-Embedding-0.6B |
| agi-agent | 8080 | Main API (optional) |

## Troubleshooting

### "Agent: Not running"
This is fine - chat still works via direct llama.cpp. To enable memory/sessions, start the Agent.

```bash
sudo systemctl start agi-agent
```

### Connection refused
```bash
# Check llama.cpp
sudo systemctl status llama-qwen

# Check Agent
curl http://localhost:8080/health
```

### RAG not finding memories
```bash
# Check what's in training namespace
./agi data name

# Query directly
curl -X POST http://localhost:8080/memories/query \
  -H "Content-Type: application/json" \
  -d '{"query": "name", "namespace": "training", "limit": 5}'
```

### Training fails
```bash
# Check Python environment
source ~/.agi-venv/bin/activate
python -c "import unsloth"

# Check disk space
df -h ~/.agi/

# Check model directory
ls -la ~/.agent/models/
```
