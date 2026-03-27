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

LLAMA_URL = "http://10.10.199.146:8081"

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
        "Type 'clear' to clear the conversation"
    ])
    
    messages = [{
        "role": "system",
        "content": """You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."""
    }]
    
    while True:
        try:
            user_input = input(f"{Colors.GREEN}👤 You:{Colors.ENDC} ").strip()
        except EOFError:
            print("\n\nGoodbye!")
            break
        
        if not user_input:
            continue
        
        cmd = user_input.lower()
        if cmd in ['exit', 'quit', 'q']:
            print(f"\n{Colors.CYAN}👋 Goodbye!{Colors.ENDC}\n")
            break
        
        if cmd in ['clear', 'c']:
            messages = [{
                "role": "system",
                "content": """You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."""
            }]
            print(f"{Colors.YELLOW}✓ Conversation cleared{Colors.ENDC}")
            continue
        
        messages.append({"role": "user", "content": user_input})
        
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
            
        except requests.exceptions.RequestException as e:
            print(f"{Colors.RED}\n❌ Error: {e}{Colors.ENDC}")
            messages.pop()  # Remove failed user message


def train(steps: int = 500, epochs: int = 3):
    """Trigger RL training"""
    print_box("RL Training Trigger", [
        f"Steps: {steps}",
        f"Epochs: {epochs}",
        f"Base model: qwen3.5-4b"
    ])
    
    # Prepare training data
    print(f"{Colors.YELLOW}📋 Preparing training data...{Colors.ENDC}")
    training_data = prepare_training_data()
    
    if training_data:
        print(f"{Colors.GREEN}✓ Found {len(training_data)} training examples{Colors.ENDC}")
    else:
        print(f"{Colors.YELLOW}⚠️  No training data found, using default settings{Colors.ENDC}")
    
    print(f"\n{Colors.GREEN}🚀 Starting RL training...{Colors.ENDC}")
    
    # Try Ollama fine-tune API
    try:
        output_name = f"qwen3.5-4b-trained-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
        
        payload = {
            "model": "qwen3.5-4b",
            "adapter": "lora",
            "steps": steps,
            "epochs": epochs,
            "trainFiles": [],
            "output": {
                "name": output_name
            },
            "loraConfig": {
                "rank": 16,
                "alpha": 16,
                "dropout": 0.05,
                "targetModules": ["q_proj", "k_proj", "v_proj", "o_proj"]
            }
        }
        
        print(f"   Job name: {output_name}")
        
        response = requests.post(
            f"{LLAMA_URL}/api/finetune",
            json=payload,
            timeout=30
        )
        response.raise_for_status()
        
        result = response.json()
        job_id = result.get("job_id", "unknown")
        
        print(f"{Colors.GREEN}✅ Training job started!{Colors.ENDC}")
        print(f"   Job ID: {job_id}")
        print(f"\n{Colors.CYAN}📊 Monitoring training progress...{Colors.ENDC}")
        print("   (Press Ctrl+C to stop monitoring)\n")
        
        # Monitor progress
        monitor_training(job_id)
        
    except requests.exceptions.RequestException as e:
        print(f"\n{Colors.RED}❌ Failed to start training: {e}{Colors.ENDC}\n")
        print(f"{Colors.CYAN}💡 Note:{Colors.ENDC} Ollama may not support fine-tuning directly.")
        print("   Consider using a dedicated training pipeline with:")
        print("   - llama.cpp for GGUF models")
        print("   - Axolotl for fine-tuning")
        print("   - unsloth for fast LoRA training")


def prepare_training_data() -> list:
    """Prepare training data from memory store"""
    examples = []
    training_dir = Path(".agent/training_data")
    
    if training_dir.exists():
        for entry in training_dir.glob("*.json"):
            try:
                with open(entry) as f:
                    examples.append(json.load(f))
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
    
    # Check Ollama for active jobs
    try:
        response = requests.get(f"{LLAMA_URL}/api/finetune", timeout=10)
        if response.status_code == 200:
            data = response.json()
            jobs = data.get("jobs", [])
            if jobs:
                for job in jobs:
                    print(f"  Model: {job.get('model', 'unknown')}")
                    print(f"  State: {job.get('state', 'unknown')}")
            else:
                print("  No active training jobs")
    except requests.exceptions.RequestException:
        print("  Could not connect to Ollama")
    
    # List trained models
    print(f"\n{Colors.BOLD}Trained Models:{Colors.ENDC}")
    models_dir = Path(".agent/models")
    
    if models_dir.exists():
        trained = [d for d in models_dir.iterdir() if d.is_dir()]
        if trained:
            for model in trained:
                print(f"  ✓ {model.name}")
        else:
            print("  No trained models found yet")
    else:
        print("  No trained models found yet")
    
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
