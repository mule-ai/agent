# AGI Agent Summary

**Last Updated:** 2026-03-28

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| Agent API | ✅ Running | Port 8080, systemd service |
| Qwen3.5-4B | ✅ Running | Symlink: `~/qwen35-trained-latest.gguf` |
| Qwen3-Embedding | ✅ Running | Port 8083 |
| Qwen3-Reranker | ✅ Running | Port 8084 |
| RAG | ✅ Working | SQLite text search, < 3s response |
| Memory | ✅ Working | 1193 training, 123 retrieval memories |
| Training | ✅ Available | `./agi train` or API |

## What Works

1. **Chat with RAG** - Agent retrieves memories and answers questions
   - "What is my name?" → "Your name is Jeremiah"
   - "What am I learning?" → "Rust Programming Language"

2. **Memory System** - Facts moved to training on eviction (not deleted)

3. **Training Pipeline** - Manual training via CLI
   ```bash
   ./agi train  # Full pipeline: collect → train → deploy
   ```

4. **Systemd Services** - All services auto-restart on failure

## Key Files

- `README.md` - Main documentation
- `CLI.md` - Command reference
- `AGENTS.md` - Full system documentation
- `agent.toml` - Configuration
- `training_script.py` - Training with unsloth

## Configuration

- LLM: `http://10.10.199.146:8081`
- Embedding: `http://127.0.0.1:8083`
- Search: `https://search.butler.ooo`

## Next Steps

1. Schedule automated nightly training via cron
2. Add more training examples for better knowledge
3. Enable vector search with embeddings for semantic matching

## Architecture

```
CLI → Agent API (8080) → llama.cpp (8081, Qwen3.5-4B)
                ↓
          SQLite Memory
          (training + retrieval namespaces)
          
llama-embedding (8083) → Qwen3-Embedding-0.6B
llama-rerank (8084) → Qwen3-Reranker-0.6B
```
