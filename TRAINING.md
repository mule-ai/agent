# Training Environment Setup

This document explains how to set up the environment for training the AGI Agent model.

## Prerequisites

- NVIDIA GPU with CUDA support (for accelerated training)
- Python 3.12+ (recommended)
- ~20GB free disk space for models and training data

## Environment 1: Home Folder Virtual Environment

The agent has a pre-configured Python virtual environment at `~/venv` with all training dependencies.

### Activate the Environment

```bash
source ~/venv/bin/activate
```

### Verify Installation

```bash
python --version  # Should show Python 3.12.3
pip list | grep -E "unsloth|transformers|torch"
```

### Key Packages Installed

| Package | Version | Purpose |
|---------|---------|---------|
| `unsloth` | 2026.3.15 | Efficient LoRA fine-tuning |
| `unsloth_zoo` | 2026.3.5 | Unsloth utilities |
| `transformers` | 5.3.0 | Hugging Face transformers |
| `torch` | 2.7.0+cu128 | PyTorch with CUDA 12.8 |
| `trl` | 0.18.2 | Transformer Reinforcement Learning |
| `peft` | 0.18.1 | Parameter-Efficient Fine-Tuning |
| `bitsandbytes` | 0.49.2 | Quantization support |
| `flash_attn` | 2.8.3 | Flash Attention |
| `datasets` | 4.3.0 | Hugging Face datasets |
| `accelerate` | 1.6.0 | Distributed training |

## Running Training

### Option A: Via Agent API

```bash
# Trigger training via API
curl -X POST http://localhost:8080/training/trigger

# Check status
curl http://localhost:8080/training/status
```

### Option B: Direct Python Script

```python
#!/usr/bin/env python3
"""Example training script using unsloth."""

from unsloth import FastLanguageModel
import torch

# Load model
model, tokenizer = FastLanguageModel.from_pretrained(
    model_name="qwen3.5-4b",
    max_seq_length=2048,
    dtype=None,
    load_in_4bit=True,
)

# Add LoRA adapters
model = FastLanguageModel.get_peft_model(
    model,
    r=16,
    target_modules=["q_proj", "k_proj", "v_proj", "o_proj", 
                   "gate_proj", "up_proj", "down_proj"],
    lora_alpha=16,
    lora_dropout=0,
)

# Load and format training data...
# (JSONL with "prompt" and "completion" fields)

# Train
# ... (see unsloth documentation for full training code)
```

## Training Data Format

Training data should be in JSONL format with the following structure:

```jsonl
{"prompt": "User question or instruction", "completion": "Expected response"}
{"prompt": "What is Rust?", "completion": "Rust is a systems programming language..."}
```

### Quality Filtering

The agent uses quality scores to filter training examples:
- `quality_score >= 0.5`: Included in training
- Higher scores = better quality examples

## Troubleshooting

### "unsloth not installed"

```bash
source ~/venv/bin/activate
pip install unsloth
```

### CUDA out of memory

Reduce batch size in `agent.toml`:
```toml
[training]
batch_size = 2  # or 1
```

### No training data found

The training system expects training examples in the `training` namespace of memory. See PLAN.md for details on how training data should be generated from conversations.

## Model Output

Trained LoRA adapters are saved to:
```
/data/jbutler/mule/agent/.agent/models/<model-id>/
```

To use a trained model, hot-swap via API:
```bash
curl -X POST http://localhost:8080/model/update \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen3.5-4b", "adapter": "/data/jbutler/mule/agent/.agent/models/model-id"}'
```
