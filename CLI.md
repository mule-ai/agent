# AGI Agent CLI

Interactive CLI for chatting with qwen3.5-4b and triggering RL training.

## Prerequisites

- **llama.cpp server** must be running on `10.10.199.146:8081`
- Model: **Qwen3.5-4B-GGUF:Q8_0** (~4.5 GB)
- **unsloth** for RL training (installed in `/home/administrator/.venv`)

```bash
# Check if server is running
sudo systemctl status llama-qwen

# If not, start it
sudo systemctl start llama-qwen
```

## Usage

### Chat with the base model

```bash
python3 cli.py chat
# or
./agi chat
```

### Chat commands (during chat)

- `exit` / `quit` / `q` - End session and save for training
- `clear` / `c` - Clear conversation
- `save` / `s` - Save conversation for training

### Trigger RL training

```bash
# Default: 500 steps
python3 cli.py train
./agi train

# Custom settings
python3 cli.py train --steps 1000
./agi train --steps 1000
```

### Check training status

```bash
python3 cli.py status
./agi status
```

### List available models

```bash
python3 cli.py models
./agi models
```

## Workflow

### 1. Chat and Collect Data

```bash
./agi chat
# Chat with the model...
# Type 'exit' to save conversation
```

Conversations are automatically saved to:
- `.agent/conversations/` - Full conversation logs
- `.agent/training_data/` - Individual training examples

### 2. Trigger RL Training

```bash
./agi train
```

This uses **unsloth GRPO** (Group Relative Policy Optimization) to fine-tune the model with your collected conversations.

### 3. Chat with Trained Model

After training completes, the LoRA adapter is saved to:
```
~/.agent/trained_models/qwen35-4b-YYYYMMDD-HHMMSS/
```

To use with llama.cpp, you'll need to merge the LoRA adapter with the base model.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     CLI (cli.py)                            │
│    - Interactive chat with streaming                       │
│    - Saves conversations for training                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              llama.cpp server (port 8081)                   │
│    - Qwen3.5-4B-Q8_0 inference                           │
│    - Streaming responses                                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ (on exit)
┌─────────────────────────────────────────────────────────────┐
│              Training Data Storage                          │
│    .agent/conversations/  - Full logs                     │
│    .agent/training_data/  - User→Assistant pairs          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ (on train)
┌─────────────────────────────────────────────────────────────┐
│              unsloth GRPO Training                         │
│    - Uses HuggingFace model (unsloth/Qwen3.5-4B)         │
│    - Outputs LoRA adapter                                  │
└─────────────────────────────────────────────────────────────┘
```

## Configuration

Edit `agent.toml` to configure:
- Model API endpoint
- Memory storage path
- Training parameters

## Services

| Service | Port | Model |
|---------|------|-------|
| llama-qwen | 8081 | Qwen3.5-4B-Q8_0 |
| llama (GLM) | 8080 | GLM-4.7-Flash |

## Troubleshooting

### Connection refused error
Make sure llama.cpp server is running:
```bash
sudo systemctl status llama-qwen
```

### Model not loaded
Check model download:
```bash
ls -lh /home/administrator/.cache/llama.cpp/
```

### Training fails
Ensure unsloth is installed:
```bash
source /home/administrator/.venv/bin/activate
pip install unsloth trl
```
