#!/usr/bin/env python3
"""
AGI Agent CLI - Interactive Chat and RL Training

Usage:
    python cli.py chat                    # Chat with base model (qwen3.5-4b)
    python cli.py chat --model MODEL      # Chat with specific model
    python cli.py train --steps 500       # Trigger RL training
    python cli.py status                 # Check training status
    python cli.py models                 # List available models
    python cli.py chat-adapter ADAPTER   # Chat with a LoRA adapter
"""

import argparse
import json
import os
import sys
import time
import requests
from pathlib import Path
from datetime import datetime

# Agent API server (handles memory, sessions, training)
AGENT_URL = "http://localhost:8080"

# Direct llama.cpp for chat
LLAMA_URL = "http://10.10.199.146:8081"

# Storage for conversations (for training)
CONVERSATIONS_DIR = Path(".agent/conversations")
TRAINING_DATA_DIR = Path(".agent/training_data")

# ANSI colors
class Colors:
    HEADER = '\033[95m'
    BLUE = '\033[94m'
    CYAN = '\033[96m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'


def print_box(title: str, lines: list[str] = None):
    """Print a nice box around text"""
    width = 60
    print(f"\n{Colors.CYAN}╔{'═' * (width - 2)}╗{Colors.ENDC}")
    print(f"{Colors.CYAN}║{Colors.ENDC} {title.center(width - 4)} {Colors.CYAN}║{Colors.ENDC}")
    if lines:
        for line in lines:
            print(f"{Colors.CYAN}╠{'─' * (width - 2)}╣{Colors.ENDC}")
            for i in range(0, len(line), width - 4):
                chunk = line[i:i + width - 4]
                print(f"{Colors.CYAN}║{Colors.ENDC} {chunk.ljust(width - 4)} {Colors.CYAN}║{Colors.ENDC}")
    print(f"{Colors.CYAN}╚{'═' * (width - 2)}╝{Colors.ENDC}\n")


def chat(model: str = "qwen3.5-4b", adapter: str = None):
    """Interactive chat with a model"""
    print_box("AGI Agent Chat - Interactive Mode", [
        f"Model: {model}",
        "Type 'exit' or 'quit' to end the session",
        "Type 'clear' to clear the conversation",
        "Type 'save' to save for training"
    ])
    
    messages = [{
        "role": "system",
        "content": """You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."""
    }]
    
    session_id = datetime.now().strftime("%Y%m%d_%H%M%S")
    total_messages = 0
    
    while True:
        try:
            user_input = input(f"{Colors.GREEN}👤 You:{Colors.ENDC} ").strip()
        except EOFError:
            # Save on Ctrl+D
            save_conversation(messages, session_id, total_messages)
            print("\n\nGoodbye! (saved)")
            break
        
        if not user_input:
            continue
        
        cmd = user_input.lower()
        if cmd in ['exit', 'quit', 'q']:
            save_conversation(messages, session_id, total_messages)
            print(f"\n{Colors.CYAN}👋 Goodbye! (conversation saved){Colors.ENDC}\n")
            break
        
        if cmd in ['clear', 'c']:
            messages = [{
                "role": "system",
                "content": """You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."""
            }]
            total_messages = 0
            print(f"{Colors.YELLOW}✓ Conversation cleared{Colors.ENDC}")
            continue
        
        if cmd in ['save', 's']:
            save_conversation(messages, session_id, total_messages)
            print(f"{Colors.GREEN}✓ Conversation saved for training{Colors.ENDC}")
            continue
        
        messages.append({"role": "user", "content": user_input})
        total_messages += 1
        
        print(f"{Colors.BLUE}🤖 Assistant:{Colors.ENDC} ", end="", flush=True)
        
        try:
            payload = {
                "model": model,
                "messages": messages,
                "stream": True,
            }
            
            if adapter:
                payload["adapter"] = adapter
            
            # Use curl for true streaming
            import subprocess
            curl_cmd = [
                "curl", "-s", "-N", "-X", "POST",
                f"{LLAMA_URL}/v1/chat/completions",
                "-H", "Content-Type: application/json",
                "-d", json.dumps(payload)
            ]
            
            process = subprocess.Popen(
                curl_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL
            )
            
            full_content = ""
            while True:
                line = process.stdout.readline()
                if not line:
                    break
                line_text = line.decode('utf-8', errors='ignore')
                if line_text.startswith("data: "):
                    data = line_text[6:]
                    if data.strip() == "[DONE]":
                        break
                    try:
                        chunk = json.loads(data)
                        delta = chunk.get("choices", [{}])[0].get("delta", {})
                        # Only show actual content, skip reasoning/thinking
                        content_delta = delta.get("content") or ""
                        if content_delta:
                            print(content_delta, end="", flush=True)
                            full_content += content_delta
                    except json.JSONDecodeError:
                        continue
            
            process.terminate()
            print()  # newline after streaming done
            messages.append({"role": "assistant", "content": full_content})
            total_messages += 1
            
        except Exception as e:
            print(f"{Colors.RED}\n❌ Error: {e}{Colors.ENDC}")
            if messages and messages[-1]["role"] == "user":
                messages.pop()
                total_messages -= 1


def save_conversation(messages: list, session_id: str, message_count: int):
    """Save conversation to .agent/conversations/ and generate training data"""
    if message_count < 2:
        return
    
    # Create directories
    CONVERSATIONS_DIR.mkdir(parents=True, exist_ok=True)
    TRAINING_DATA_DIR.mkdir(parents=True, exist_ok=True)
    
    # Save full conversation
    conv_data = {
        "id": session_id,
        "created_at": datetime.now().isoformat(),
        "message_count": message_count,
        "messages": messages
    }
    
    conv_file = CONVERSATIONS_DIR / f"conv_{session_id}.json"
    with open(conv_file, 'w') as f:
        json.dump(conv_data, f, indent=2)
    
    # Generate training examples from user-assistant pairs
    examples_created = 0
    for i, msg in enumerate(messages):
        if msg["role"] == "user" and i + 1 < len(messages) and messages[i + 1]["role"] == "assistant":
            user_msg = msg["content"]
            assistant_msg = messages[i + 1]["content"]
            
            # Skip very short or very long messages
            if len(user_msg) < 5 or len(assistant_msg) < 10:
                continue
            
            example = {
                "id": f"{session_id}_ex_{examples_created}",
                "prompt": user_msg,
                "completion": assistant_msg,
                "source": "conversation",
                "created_at": datetime.now().isoformat()
            }
            
            example_file = TRAINING_DATA_DIR / f"ex_{session_id}_{examples_created}.json"
            with open(example_file, 'w') as f:
                json.dump(example, f)
            
            examples_created += 1
    
    print(f"  → Saved {conv_file.name} ({message_count} messages, {examples_created} training examples)")


def train(steps: int = 500, epochs: int = 3):
    """Trigger RL training using unsloth GRPO"""
    print_box("RL Training Trigger", [
        f"Steps: {steps}",
        f"Epochs: {epochs}",
        f"Base model: unsloth/Qwen3.5-4B (HuggingFace)"
    ])
    
    # Prepare training data
    print(f"{Colors.YELLOW}📋 Preparing training data...{Colors.ENDC}")
    training_data = prepare_training_data()
    
    if training_data:
        print(f"{Colors.GREEN}✓ Found {len(training_data)} training examples{Colors.ENDC}")
    else:
        print(f"{Colors.YELLOW}⚠️  No training data found.{Colors.ENDC}")
        print("   Chat with the model first, then run 'train' to fine-tune.")
        return
    
    # Generate training script with actual data
    print(f"\n{Colors.CYAN}🔧 Generating training script...{Colors.ENDC}")
    output_dir = Path.home() / ".agent" / "trained_models" / f"qwen35-4b-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Convert training data to prompts format
    prompts = [ex["prompt"] for ex in training_data[:100]]  # Limit to 100 examples
    
    # Generate custom training script
    script_content = f'''#!/usr/bin/env python3
"""
Auto-generated GRPO Training Script
Generated: {datetime.now().isoformat()}
"""

import torch
from unsloth import FastLanguageModel
from trl import GRPOConfig, GRPOTrainer
from datasets import Dataset

CONFIG = {{
    "output_dir": "{output_dir}",
    "max_steps": {steps},
    "learning_rate": 2e-5,
    "lora_r": 16,
}}

print(f"=" * 60)
print("Qwen3.5-4B GRPO Training")
print(f"Examples: {{len(prompts)}}")
print(f"Output: {{CONFIG['output_dir']}}")
print("=" * 60)

# Load model
model, tokenizer = FastLanguageModel.from_pretrained(
    model_name="unsloth/Qwen3.5-4B",
    max_seq_length=2048,
    load_in_8bit=True,
    load_in_4bit=False,
)

# Attach LoRA
model = FastLanguageModel.get_peft_model(
    model,
    r=CONFIG["lora_r"],
    target_modules=["q_proj", "k_proj", "v_proj", "o_proj"],
)

# Reward function
def reward_function(completions, **kwargs):
    rewards = []
    for c in completions:
        score = 0.0
        if len(c) > 10:
            score += 1.0
        if c.strip().endswith(('.', '!', '?', ')')):
            score += 0.5
        if "\\n" in c:
            score += 0.5
        rewards.append(score)
    return rewards

# Dataset
def format_prompt(prompt):
    return f"<|im_start|>user\\n{{prompt}}<|im_end|>\\n<|im_start|>assistant\\n"

prompts = {prompts}

dataset = Dataset.from_dict({{
    "prompt": [format_prompt(p) for p in prompts],
    "completion": [""],
}})

# Train
trainer = GRPOTrainer(
    model=model,
    processing_class=tokenizer,
    reward_functions=[reward_function],
    args=GRPOConfig(
        output_dir=CONFIG["output_dir"],
        max_steps=CONFIG["max_steps"],
        learning_rate=CONFIG["learning_rate"],
        per_device_train_batch_size=2,
        gradient_accumulation_steps=4,
        warmup_steps=10,
        logging_steps=10,
        save_steps=50,
    ),
    train_dataset=dataset,
)

print("Starting training...")
trainer.train()

print("Saving model...")
trainer.save_model(CONFIG["output_dir"])
tokenizer.save_pretrained(CONFIG["output_dir"])
print(f"Done! Saved to {{CONFIG['output_dir']}}")
'''
    
    script_path = output_dir / "train.py"
    with open(script_path, 'w') as f:
        f.write(script_content)
    
    print(f"   Script: {script_path}")
    print(f"   Output: {output_dir}")
    
    # Check if unsloth is available
    print(f"\n{Colors.GREEN}🚀 Starting training...{Colors.ENDC}")
    print(f"   Run manually: python3 {script_path}")
    print()
    
    try:
        import subprocess
        result = subprocess.run(
            ["python3", str(script_path)],
            capture_output=False,
            timeout=3600 * 2  # 2 hour timeout
        )
        
        if result.returncode == 0:
            print(f"\n{Colors.GREEN}✅ Training complete!{Colors.ENDC}")
            print(f"   Model saved: {output_dir}")
            
            # Create merged model for llama.cpp
            merged_path = output_dir / "merged"
            print(f"\n{Colors.CYAN}📦 To use with llama.cpp:{Colors.ENDC}")
            print(f"   1. Export: python3 -m unsloth {output_dir}")
            print(f"   2. Or merge LoRA:")
            print(f"      from unsloth import FastLanguageModel")
            print(f"      model.save_pretrained_merged('{merged_path}', tokenizer, save_method='merged_16bit')")
        else:
            print(f"\n{Colors.RED}❌ Training failed{Colors.ENDC}")
            
    except KeyboardInterrupt:
        print(f"\n{Colors.YELLOW}⚠️  Training interrupted{Colors.ENDC}")
    except Exception as e:
        print(f"\n{Colors.YELLOW}⚠️  Could not run training: {e}{Colors.ENDC}")
        print(f"   Run manually: python3 {script_path}")


def prepare_training_data() -> list:
    """Prepare training data from saved conversations"""
    examples = []
    training_dir = TRAINING_DATA_DIR
    
    if training_dir.exists():
        for entry in training_dir.glob("*.json"):
            try:
                with open(entry) as f:
                    data = json.load(f)
                    # Each file is one training example
                    if "prompt" in data and "completion" in data:
                        examples.append(data)
            except (json.JSONDecodeError, IOError):
                pass
    
    return examples


def monitor_training(job_id: str):
    """Monitor training progress"""
    try:
        while True:
            try:
                response = requests.get(f"{LLAMA_URL}/api/finetune/{job_id}", timeout=10)
                
                if response.status_code == 200:
                    status = response.json()
                    state = status.get("state", "unknown")
                    progress = status.get("progress", 0)
                    loss = status.get("loss")
                    
                    loss_str = f" | Loss: {loss:.4f}" if loss else ""
                    print(f"\r{Colors.CYAN}📈{Colors.ENDC} Status: {state:12} | Progress: {progress:5.1f}%{loss_str}    ", end="", flush=True)
                    
                    if state in ["completed", "failed", "cancelled"]:
                        print()  # newline
                        break
                else:
                    print(f"\r{Colors.YELLOW}⚠️{Colors.ENDC} Status check failed, retrying...", end="", flush=True)
                    
            except requests.exceptions.RequestException:
                print(f"\r{Colors.YELLOW}⚠️{Colors.ENDC} Connection lost, retrying...", end="", flush=True)
            
            time.sleep(5)
        
        print(f"\n{Colors.GREEN}✅ Training monitoring complete!{Colors.ENDC}\n")
        
    except KeyboardInterrupt:
        print(f"\n\n{Colors.YELLOW}⚠️  Stopped monitoring (training may continue){Colors.ENDC}\n")


def status():
    """Show training status"""
    print_box("Training Status")
    
    # Count training data
    training_data = prepare_training_data()
    conv_count = len(list(CONVERSATIONS_DIR.glob("*.json"))) if CONVERSATIONS_DIR.exists() else 0
    example_count = len(training_data)
    
    print(f"  Conversations: {Colors.GREEN}{conv_count}{Colors.ENDC}")
    print(f"  Training examples: {Colors.GREEN}{example_count}{Colors.ENDC}")
    
    if example_count > 0:
        print(f"\n  {Colors.CYAN}Run './agi train' to fine-tune the model{Colors.ENDC}")
    
    # Check trained models
    print(f"\n{Colors.BOLD}Trained Models:{Colors.ENDC}")
    trained_dir = Path.home() / ".agent" / "trained_models"
    if trained_dir.exists():
        models = list(trained_dir.iterdir())
        if models:
            for m in models:
                print(f"  ✓ {m.name}")
        else:
            print("  No trained models yet")
    else:
        print("  No trained models yet")
    
    print()


def models():
    """List available models"""
    print_box("Available Models")
    
    try:
        response = requests.get(f"{LLAMA_URL}/api/tags", timeout=10)
        response.raise_for_status()
        
        data = response.json()
        model_list = data.get("models", [])
        
        if model_list:
            print(f"{'Model':<40} {'Size':<12} {'Type'}")
            print("-" * 60)
            
            for model in model_list:
                name = model.get("name", "unknown")
                size = model.get("size", 0)
                size_gb = size / (1024**3)
                
                is_base = "qwen3.5-4b" in name and "trained" not in name
                model_type = "[BASE]" if is_base else "[     ]"
                
                print(f"{name:<40} {size_gb:>6.1f} GB  {model_type}")
        else:
            print("No models found")
            
    except requests.exceptions.RequestException as e:
        print(f"Error connecting to Ollama: {e}")
    
    print()
    print("  [BASE] = Base model (qwen3.5-4b)")
    print()
    print("  Commands:")
    print("    Chat with base model:   python cli.py chat")
    print("    Chat with specific:     python cli.py chat --model MODEL")
    print("    Trigger RL training:    python cli.py train")
    print()


def main():
    parser = argparse.ArgumentParser(
        description="AGI Agent CLI - Interactive Chat and RL Training",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python cli.py chat                    Chat with base model
  python cli.py chat --model llama3     Chat with specific model
  python cli.py train --steps 500       Trigger RL training
  python cli.py status                  Check training status
  python cli.py models                  List available models
        """
    )
    
    subparsers = parser.add_subparsers(dest="command", help="Available commands")
    
    # Chat command
    chat_parser = subparsers.add_parser("chat", help="Start interactive chat session")
    chat_parser.add_argument("--model", "-m", default="qwen3.5-4b", help="Model name to chat with")
    
    # Train command
    train_parser = subparsers.add_parser("train", help="Trigger RL training")
    train_parser.add_argument("--steps", "-s", type=int, default=500, help="Number of training steps")
    train_parser.add_argument("--epochs", "-e", type=int, default=3, help="Number of epochs")
    
    # Status command
    subparsers.add_parser("status", help="Check training status")
    
    # Models command
    subparsers.add_parser("models", help="List available models")
    
    # Chat adapter command
    adapter_parser = subparsers.add_parser("chat-adapter", help="Chat with a LoRA adapter")
    adapter_parser.add_argument("adapter", help="Name of the LoRA adapter")
    
    args = parser.parse_args()
    
    if args.command == "chat":
        chat(model=args.model)
    elif args.command == "train":
        train(steps=args.steps, epochs=args.epochs)
    elif args.command == "status":
        status()
    elif args.command == "models":
        models()
    elif args.command == "chat-adapter":
        chat(model="qwen3.5-4b", adapter=args.adapter)
    else:
        # Default to chat if no command
        chat()


if __name__ == "__main__":
    main()
