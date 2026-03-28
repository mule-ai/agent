"""
Qwen3.5-4B GRPO/LoRA Training Script for AGI Agent
==================================================
Trains on accumulated examples using GRPO with unsloth
"""

import json
import os
import sys
import re
from pathlib import Path

import torch
from unsloth import FastLanguageModel
from datasets import Dataset
from transformers import TrainingArguments
from trl import SFTTrainer

# ============================================
# REWARD FUNCTIONS (for GRPO)
# ============================================
REASONING_START = "<REASONING>"
REASONING_END = "</REASONING>"
SOLUTION_START = "<SOLUTION>"
SOLUTION_END = "</SOLUTION>"

def formatting_reward_func(completions, **kwargs):
    """Rewards proper use of reasoning and answer tags."""
    thinking_pattern = f'{REASONING_START}(.*?){REASONING_END}'
    answer_pattern = f'{SOLUTION_START}(.*?){SOLUTION_END}'
    
    scores = []
    for completion in completions:
        if isinstance(completion, list):
            completion = completion[0]["content"] if completion else ""
        
        score = 0
        thinking_matches = re.findall(thinking_pattern, completion, re.DOTALL)
        answer_matches = re.findall(answer_pattern, completion, re.DOTALL)
        
        if len(thinking_matches) == 1:
            score += 1.0
        if len(answer_matches) == 1:
            score += 1.0
        
        scores.append(score)
    return scores

def correctness_reward_func(prompts, completions, answer, **kwargs):
    """Rewards correct numerical answers."""
    answer_pattern = f'{SOLUTION_START}(.*?){SOLUTION_END}'
    
    completions = [(c[0]["content"] if c else "") if isinstance(c, list) else c for c in completions]
    responses = [re.findall(answer_pattern, completion, re.DOTALL) for completion in completions]
    
    return [
        2.0 if len(r) == 1 and str(a) == r[0].replace('\n', '').strip() else 0.0
        for r, a in zip(responses, answer)
    ]

def main():
    # Get paths from arguments
    if len(sys.argv) < 3:
        print(json.dumps({"status": "error", "reason": "Usage: training_script.py <data_path> <output_dir>"}))
        sys.exit(1)
    
    data_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    print("=" * 60)
    print("Qwen3.5-4B LoRA Training")
    print("=" * 60)
    print(f"GPU: {torch.cuda.get_device_name(0)}")
    print(f"VRAM: {torch.cuda.get_device_properties(0).total_memory / 1024**3:.1f} GB")
    print(f"Data: {data_path}")
    print(f"Output: {output_dir}")
    print("=" * 60)
    
    # Load training data
    print("\n[1/5] Loading training data...")
    try:
        with open(data_path, "r") as f:
            data = [json.loads(line) for line in f]
        print(f"Loaded {len(data)} training examples")
    except Exception as e:
        print(json.dumps({"status": "error", "reason": f"Failed to load data: {e}"}))
        sys.exit(1)
    
    # Load model - Qwen3.5-4B from unsloth GGUF
    print("\n[2/5] Loading Qwen3.5-4B model from HuggingFace...")
    
    # Use unsloth's Qwen3.5-4B GGUF model
    model_name = "unsloth/Qwen3.5-4B-GGUF"
    
    try:
        model, tokenizer = FastLanguageModel.from_pretrained(
            model_name=model_name,
            max_seq_length=2048,
            load_in_4bit=True,
        )
        print(f"Model loaded! VRAM: {torch.cuda.memory_allocated() / 1024**3:.2f} GB")
    except Exception as e:
        print(f"Model load failed: {e}")
        print("Trying Qwen2.5-0.5B as fallback...")
        model, tokenizer = FastLanguageModel.from_pretrained(
            model_name="unsloth/Qwen2.5-0.5B-Instruct",
            max_seq_length=2048,
            load_in_4bit=True,
        )
        print(f"Fallback model loaded! VRAM: {torch.cuda.memory_allocated() / 1024**3:.2f} GB")
    
    # Add LoRA
    print("\n[3/5] Adding LoRA adapters...")
    model = FastLanguageModel.get_peft_model(
        model,
        r=16,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
        lora_alpha=16,
        lora_dropout=0,
        bias="none",
        use_gradient_checkpointing=True,
    )
    model.print_trainable_parameters()
    print(f"LoRA added! VRAM: {torch.cuda.memory_allocated() / 1024**3:.2f} GB")
    
    # Prepare dataset
    print("\n[4/5] Preparing dataset...")
    
    # Filter to valid training examples
    valid_data = []
    for item in data:
        if isinstance(item, dict) and "prompt" in item and "completion" in item:
            # Skip placeholder/test data
            if item.get("prompt") != "Test prompt" and item.get("completion") != "Test completion":
                valid_data.append(item)
    
    if not valid_data:
        print("No valid training examples found, using all data...")
        valid_data = data[:100]  # Limit to 100 for testing
    
    # Format for SFT training
    def format_example(example):
        prompt = example.get("prompt", "")
        completion = example.get("completion", "")
        reasoning = example.get("reasoning", "")
        
        if reasoning:
            text = f"{REASONING_START}{reasoning}{REASONING_END}\n{SOLUTION_START}{completion}{SOLUTION_END}"
        else:
            text = f"{SOLUTION_START}{completion}{SOLUTION_END}"
        
        return {"text": prompt + "\n\n" + text}
    
    formatted_data = [format_example(d) for d in valid_data]
    dataset = Dataset.from_list(formatted_data)
    print(f"Dataset prepared: {len(dataset)} examples")
    
    # Configure training
    print("\n[5/5] Configuring training...")
    
    training_args = TrainingArguments(
        output_dir=output_dir,
        per_device_train_batch_size=2,
        gradient_accumulation_steps=4,
        warmup_steps=2,
        num_train_epochs=1,
        learning_rate=2e-4,
        fp16=not torch.cuda.is_bf16_supported(),
        logging_steps=10,
        optim="adamw_8bit",
        weight_decay=0.01,
        lr_scheduler_type="linear",
        seed=3407,
        save_steps=50,
        save_total_limit=1,
    )
    
    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=dataset,
        dataset_text_field="text",
        max_seq_length=2048,
        dataset_num_proc=4,
        packing=True,
        args=training_args,
    )
    
    print("\n" + "=" * 60)
    print("Starting Training!")
    print("=" * 60)
    
    trainer.train()
    
    # Save
    print("\nSaving model...")
    os.makedirs(output_dir, exist_ok=True)
    model.save_pretrained(output_dir)
    tokenizer.save_pretrained(output_dir)
    
    # Create config
    config = {
        "model_id": os.path.basename(output_dir),
        "metrics": {
            "samples": len(valid_data),
            "train_loss": trainer.state.log_history[-1].get("loss", 0) if trainer.state.log_history else 0,
        }
    }
    with open(os.path.join(output_dir, "config.json"), "w") as f:
        json.dump(config, f, indent=2)
    
    print("\n" + "=" * 60)
    print("Training complete!")
    print(f"Model saved: {output_dir}")
    print(f"Final VRAM: {torch.cuda.memory_allocated() / 1024**3:.2f} GB")
    print("=" * 60)
    
    print(json.dumps({"status": "success", "samples": len(valid_data)}))

if __name__ == "__main__":
    main()
