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

# Agent API server (Rust Agent handles all logic)
AGENT_URL = "http://localhost:8080"

# llama.cpp (called by Rust Agent, not directly by CLI)
LLAMA_URL = "http://10.10.199.146:8081"

# Storage (handled by Rust Agent - these are just for display)
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


def chat(model: str = "agent"):
    """Interactive chat - tries Agent API first, falls back to direct llama.cpp"""
    
    # Check if Agent is available
    agent_available = check_agent_available()
    
    if agent_available:
        print_box("AGI Agent Chat", [
            "Model: qwen3.5-4b (via Rust Agent)",
            "Agent: Connected ✓",
            "Type 'exit' or 'quit' to end the session",
            "Type 'clear' to clear the conversation"
        ])
    else:
        print_box("AGI Agent Chat", [
            "Model: qwen3.5-4b (direct llama.cpp)",
            "Agent: Not running (memory/sessions disabled)",
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
                "stream": False,
            }
            
            # Choose endpoint based on Agent availability
            url = f"{AGENT_URL}/v1/chat/completions" if agent_available else f"{LLAMA_URL}/v1/chat/completions"
            
            # Use curl subprocess
            import subprocess
            curl_cmd = [
                "curl", "-s", "-X", "POST",
                url,
                "-H", "Content-Type: application/json",
                "-d", json.dumps(payload)
            ]
            
            result = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=120)
            response_json = json.loads(result.stdout)
            
            content = response_json.get("choices", [{}])[0].get("message", {}).get("content", "")
            print(content)
            messages.append({"role": "assistant", "content": content})
            
        except Exception as e:
            print(f"{Colors.RED}\n❌ Error: {e}{Colors.ENDC}")
            if messages and messages[-1]["role"] == "user":
                messages.pop()


def check_agent_available():
    """Check if the Rust Agent API is running"""
    try:
        response = requests.get(f"{AGENT_URL}/health", timeout=2)
        return response.status_code == 200
    except:
        return False


def check_agent_available():
    """Check if the Rust Agent API is running"""
    try:
        response = requests.get(f"{AGENT_URL}/health", timeout=2)
        return response.status_code == 200
    except:
        return False


def get_agent_stats():
    """Get stats from Agent API"""
    try:
        response = requests.get(f"{AGENT_URL}/memories/stats", timeout=5)
        if response.status_code == 200:
            return response.json()
    except:
        pass
    return None


def train(steps: int = 500, epochs: int = 3):
    """Trigger RL training via Agent API"""
    print_box("RL Training Trigger", [
        f"Steps: {steps}",
        f"Epochs: {epochs}",
        "Rust Agent handles all training logic"
    ])
    
    try:
        payload = {"steps": steps, "epochs": epochs}
        response = requests.post(
            f"{AGENT_URL}/training/trigger",
            json=payload,
            timeout=30
        )
        
        if response.status_code == 200:
            result = response.json()
            print(f"{Colors.GREEN}✅ Training started!{Colors.ENDC}")
            print(f"   Job ID: {result.get('job_id', 'unknown')}")
            print(f"\n{Colors.CYAN}Check status: ./agi status{Colors.ENDC}")
        elif response.status_code == 409:
            print(f"{Colors.YELLOW}⚠️  Training already in progress{Colors.ENDC}")
            print(f"   Check status: ./agi status")
        else:
            print(f"{Colors.RED}❌ Failed to start training{Colors.ENDC}")
            
    except requests.exceptions.RequestException as e:
        print(f"{Colors.RED}❌ Could not connect to Agent: {e}{Colors.ENDC}")
        print(f"   Make sure the Agent is running on {AGENT_URL}")


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
    """Show agent status via API"""
    print_box("Agent Status")
    
    # Get memory stats from Agent
    stats = get_agent_stats()
    if stats:
        total = stats.get("total_memories", 0)
        retrieval = stats.get("namespaces", {}).get("retrieval", 0)
        training = stats.get("namespaces", {}).get("training", 0)
        print(f"  {Colors.BOLD}Memory:{Colors.ENDC}")
        print(f"    Total memories: {Colors.GREEN}{total}{Colors.ENDC}")
        print(f"    Retrieval: {retrieval}")
        print(f"    Training: {training}")
    else:
        print(f"  {Colors.YELLOW}⚠️  Could not connect to Agent{Colors.ENDC}")
        print(f"    Make sure the Agent is running on {AGENT_URL}")
    
    # Get training status from Agent
    print(f"\n  {Colors.BOLD}Training:{Colors.ENDC}")
    try:
        response = requests.get(f"{AGENT_URL}/training/status", timeout=5)
        if response.status_code == 200:
            data = response.json()
            print(f"    Status: {data.get('status', 'unknown')}")
            print(f"    Total jobs: {data.get('total_jobs', 0)}")
        else:
            print(f"    No training data yet")
    except:
        print(f"    Could not get training status")
    
    # Get trained models
    print(f"\n  {Colors.BOLD}Trained Models:{Colors.ENDC}")
    try:
        response = requests.get(f"{AGENT_URL}/training/models", timeout=5)
        if response.status_code == 200:
            data = response.json()
            models = data.get("models", [])
            if models:
                for m in models:
                    print(f"    ✓ {m.get('name', 'unknown')}")
            else:
                print("    No trained models yet")
        else:
            print("    No trained models yet")
    except:
        print("    Could not get models")
    
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
