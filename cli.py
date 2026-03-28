#!/usr/bin/env python3
"""
AGI Agent CLI - Interactive Chat and RL Training

Usage:
    python cli.py chat                    # Chat with base model (qwen3.5-4b)
    python cli.py train                  # Trigger batch training
    python cli.py status                 # Check status
    python cli.py models                 # List available models
    python cli.py research <topic>       # Research a topic
"""

import argparse
import json
import sys
import time
import requests
from pathlib import Path

# Agent API server
AGENT_URL = "http://localhost:8080"

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


def check_agent():
    """Check if agent is running"""
    try:
        r = requests.get(f"{AGENT_URL}/health", timeout=2)
        if r.status_code == 200:
            return r.json()
    except:
        pass
    return None


def print_box(title, lines=None, width=60):
    """Print a nice box"""
    print(f"\n{Colors.CYAN}╔{'═' * (width - 2)}╗{Colors.ENDC}")
    print(f"{Colors.CYAN}║{Colors.ENDC} {title.center(width - 4)} {Colors.CYAN}║{Colors.ENDC}")
    if lines:
        print(f"{Colors.CYAN}╠{'─' * (width - 2)}╣{Colors.ENDC}")
        for line in lines:
            print(f"{Colors.CYAN}║{Colors.ENDC} {line.ljust(width - 4)} {Colors.CYAN}║{Colors.ENDC}")
    print(f"{Colors.CYAN}╚{'═' * (width - 2)}╝{Colors.ENDC}\n")


def chat(model="agent"):
    """Interactive chat"""
    health = check_agent()
    if not health:
        print(f"{Colors.RED}❌ Agent not running! Start with: ./agent{Colors.ENDC}\n")
        return
    
    print_box("AGI Agent Chat", [
        f"Model: {model}",
        f"Agent: v{health.get('version', '?')} ✓",
        "Type 'exit' to quit, 'clear' to clear history"
    ])
    
    messages = [{
        "role": "system",
        "content": "You are a helpful AI assistant. Be concise and informative."
    }]
    
    while True:
        try:
            user_input = input(f"{Colors.GREEN}👤 You:{Colors.ENDC} ").strip()
        except EOFError:
            break
        
        if not user_input:
            continue
        
        if user_input.lower() in ['exit', 'quit', 'q']:
            print(f"\n{Colors.CYAN}👋 Goodbye!{Colors.ENDC}\n")
            break
        
        if user_input.lower() in ['clear', 'c']:
            messages = [{"role": "system", "content": "You are a helpful AI assistant."}]
            print(f"{Colors.YELLOW}✓ Cleared{Colors.ENDC}")
            continue
        
        messages.append({"role": "user", "content": user_input})
        print(f"{Colors.BLUE}🤖 Assistant:{Colors.ENDC} ", end="", flush=True)
        
        try:
            r = requests.post(
                f"{AGENT_URL}/v1/chat/completions",
                json={"model": model, "messages": messages},
                timeout=60
            )
            data = r.json()
            content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
            print(content)
            messages.append({"role": "assistant", "content": content})
        except Exception as e:
            print(f"{Colors.RED}❌ Error: {e}{Colors.ENDC}")
            if messages and messages[-1]["role"] == "user":
                messages.pop()


def train():
    """Trigger batch training"""
    print_box("Batch Training")
    
    # Get batch stats
    try:
        r = requests.get(f"{AGENT_URL}/training/batch/stats", timeout=5)
        if r.status_code == 200:
            stats = r.json()
            print(f"  {Colors.BOLD}Training Examples:{Colors.ENDC}")
            print(f"    Collected: {stats.get('example_count', 0)}")
            print(f"    Ready: {'Yes ✓' if stats.get('is_ready') else 'No ✗'}")
            print()
    except Exception as e:
        print(f"  {Colors.YELLOW}Could not fetch stats: {e}{Colors.ENDC}\n")
    
    print(f"  {Colors.CYAN}Triggering batch training...{Colors.ENDC}")
    
    try:
        r = requests.post(
            f"{AGENT_URL}/training/batch/run",
            json={"force": True},
            timeout=30
        )
        if r.status_code == 200:
            result = r.json()
            if result.get("success"):
                print(f"  {Colors.GREEN}✅ Training started!{Colors.ENDC}")
            else:
                print(f"  {Colors.YELLOW}⚠️ {result.get('message', 'Failed')}{Colors.ENDC}")
        else:
            print(f"  {Colors.RED}❌ HTTP {r.status_code}{Colors.ENDC}")
    except Exception as e:
        print(f"  {Colors.RED}❌ Error: {e}{Colors.ENDC}")
    
    print()
    print(f"  Check status: {Colors.CYAN}./agi status{Colors.ENDC}\n")


def status():
    """Show comprehensive status"""
    health = check_agent()
    
    print_box("Agent Status")
    
    if not health:
        print(f"  {Colors.RED}❌ Agent not running{Colors.ENDC}")
        print(f"  Start with: {Colors.CYAN}./agent{Colors.ENDC}\n")
        return
    
    print(f"  {Colors.GREEN}✓{Colors.ENDC} Agent v{health.get('version', '?')} - {health.get('status', 'running')}")
    
    # Memory stats
    print(f"\n  {Colors.BOLD}Memory:{Colors.ENDC}")
    try:
        r = requests.get(f"{AGENT_URL}/memories/stats", timeout=5)
        if r.status_code == 200:
            stats = r.json()
            print(f"    Total: {stats.get('total', 0)}")
            for ns in stats.get('by_namespace', []):
                ns_name = ns.get('namespace', 'unknown')
                ns_count = ns.get('count', 0)
                print(f"    {ns_name}: {ns_count}")
            
            print(f"\n    By type:")
            for t in stats.get('by_type', []):
                print(f"      {t.get('type', 'unknown')}: {t.get('count', 0)}")
    except Exception as e:
        print(f"    {Colors.YELLOW}Could not fetch: {e}{Colors.ENDC}")
    
    # Batch training
    print(f"\n  {Colors.BOLD}Batch Training:{Colors.ENDC}")
    try:
        r = requests.get(f"{AGENT_URL}/training/batch/stats", timeout=5)
        if r.status_code == 200:
            stats = r.json()
            print(f"    Examples: {stats.get('example_count', 0)} / 50")
            ready = stats.get('is_ready', False)
            status_icon = f"{Colors.GREEN}✓{Colors.ENDC}" if ready else f"{Colors.RED}✗{Colors.ENDC}"
            print(f"    Ready: {status_icon}")
    except Exception as e:
        print(f"    {Colors.YELLOW}Could not fetch: {e}{Colors.ENDC}")
    
    # Training status
    print(f"\n  {Colors.BOLD}Training Status:{Colors.ENDC}")
    try:
        r = requests.get(f"{AGENT_URL}/training/batch/status", timeout=5)
        if r.status_code == 200:
            stats = r.json()
            print(f"    Status: {stats.get('status', 'unknown')}")
            print(f"    Models trained: {stats.get('models_trained', 0)}")
            if stats.get('last_training'):
                print(f"    Last: {stats.get('last_training', 'never')}")
    except Exception as e:
        print(f"    {Colors.YELLOW}Could not fetch: {e}{Colors.ENDC}")
    
    # Recent training memories
    print(f"\n  {Colors.BOLD}Recent Training Data:{Colors.ENDC}")
    try:
        r = requests.get(f"{AGENT_URL}/memories?namespace=training&limit=5", timeout=5)
        if r.status_code == 200:
            data = r.json()
            memories = data.get('memories', [])
            if memories:
                for m in memories:
                    content = m.get('content', '')[:60]
                    mtype = m.get('memory_type', 'unknown')
                    print(f"    • [{mtype}] {content}...")
            else:
                print(f"    {Colors.YELLOW}(none){Colors.ENDC}")
    except Exception as e:
        print(f"    {Colors.YELLOW}Could not fetch: {e}{Colors.ENDC}")
    
    print()


def research(topic):
    """Research a topic using search learning"""
    print(f"{Colors.CYAN}🔍 Researching: {topic}{Colors.ENDC}\n")
    
    try:
        r = requests.post(
            f"{AGENT_URL}/services/search-learning",
            json={"topic": topic},
            timeout=60
        )
        if r.status_code == 200:
            result = r.json()
            print(f"  {Colors.GREEN}✅ Research complete!{Colors.ENDC}")
            print(f"  Topics researched: {result.get('topics_researched', 0)}")
            print(f"  Concepts learned: {result.get('concepts_learned', 0)}")
            
            # Check updated stats
            r2 = requests.get(f"{AGENT_URL}/training/batch/stats", timeout=5)
            if r2.status_code == 200:
                stats = r2.json()
                print(f"\n  Training examples now: {stats.get('example_count', 0)}")
                if stats.get('is_ready'):
                    print(f"  {Colors.GREEN}✓ Ready for training!{Colors.ENDC}")
        else:
            print(f"  {Colors.RED}❌ Failed: HTTP {r.status_code}{Colors.ENDC}")
    except Exception as e:
        print(f"  {Colors.RED}❌ Error: {e}{Colors.ENDC}")
    
    print()


def models():
    """List available models"""
    print_box("Available Models")
    
    try:
        r = requests.get(f"{AGENT_URL}/v1/models", timeout=5)
        if r.status_code == 200:
            data = r.json()
            for model in data.get('data', []):
                name = model.get('id', 'unknown')
                print(f"  {Colors.CYAN}•{Colors.ENDC} {name}")
    except Exception as e:
        print(f"  {Colors.YELLOW}Could not fetch: {e}{Colors.ENDC}")
    
    print()
    print(f"  Commands:")
    print(f"    ./agi chat         - Chat with agent")
    print(f"    ./agi train        - Trigger training")
    print(f"    ./agi status       - Show status")
    print(f"    ./agi research <t>  - Research a topic")
    print()


def main():
    parser = argparse.ArgumentParser(description="AGI Agent CLI")
    sub = parser.add_subparsers(dest="cmd")
    
    sub.add_parser("chat", help="Start chat session")
    sub.add_parser("train", help="Trigger training")
    sub.add_parser("status", help="Show status")
    sub.add_parser("models", help="List models")
    
    research_parser = sub.add_parser("research", help="Research a topic")
    research_parser.add_argument("topic", help="Topic to research")
    
    args = parser.parse_args()
    
    if args.cmd == "chat":
        chat()
    elif args.cmd == "train":
        train()
    elif args.cmd == "status":
        status()
    elif args.cmd == "models":
        models()
    elif args.cmd == "research":
        research(args.topic)
    else:
        chat()


if __name__ == "__main__":
    main()
