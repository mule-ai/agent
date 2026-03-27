# AGI Agent CLI

Interactive CLI for chatting with qwen3.5-4b (via llama.cpp) and triggering RL training.

## Prerequisites

- **llama.cpp server** must be running on `10.10.199.146:8081`
- Model: **Qwen3.5-4B-GGUF:Q8_0** (~4.5 GB)

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

### Chat with a specific model

```bash
python3 cli.py chat --model glm-4.7-flash
./agi chat --model glm-4.7-flash
```

### Trigger RL training

```bash
# Default: 500 steps, 3 epochs
python3 cli.py train
./agi train

# Custom settings
python3 cli.py train --steps 1000 --epochs 5
./agi train --steps 1000 --epochs 5
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

### Chat with a trained LoRA adapter

```bash
python3 cli.py chat-adapter my-trained-adapter
./agi adapter my-trained-adapter
```

## Workflow

1. **Start chatting** with the base model:
   ```bash
   ./agi chat
   ```

2. **Trigger RL training** when you have good conversations:
   ```bash
   ./agi train --steps 500
   ```

3. **Chat with the newly trained model**:
   ```bash
   ./agi adapter <adapter-name>
   ```

## Interactive Commands

Inside chat mode:
- `exit` / `quit` / `q` - End the session
- `clear` / `c` - Clear conversation history

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
sudo journalctl -u llama-qwen -f
```

### Model not loaded
The service auto-downloads from HuggingFace on first run. Check:
```bash
ls -lh /home/administrator/.cache/llama.cpp/
```
