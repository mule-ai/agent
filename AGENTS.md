# AGI Agent System Documentation

## Table of Contents

1. [System Overview](#system-overview)
2. [Architecture](#architecture)
3. [Agent Core](#agent-core)
4. [Memory System](#memory-system)
5. [Tool System](#tool-system)
6. [Background Services](#background-services)
7. [Training Pipeline](#training-pipeline)
8. [API Reference](#api-reference)
9. [Configuration](#configuration)
10. [Extending the System](#extending-the-system)

---

## System Overview

The AGI Agent is built as a collection of cooperating components:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Python CLI (./agi)                                 │
│                    Simple client - calls Agent API                           │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Agent API (port 8080)                               │
│                      Rust HTTP Server (Axum)                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ Session Manager │  │  Reasoning     │  │      LLM Client            │ │
│  │ (State)        │  │   Engine       │  │      → llama.cpp           │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        llama.cpp Server (port 8081)                         │
│                      Qwen3.5-4B-GGUF:Q8_0 Model                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Architecture

### Python CLI

The Python CLI (`./agi` or `python3 cli.py`) is a simple client that:

- Calls the Agent API on port 8080
- Auto-detects if Agent is running
- Falls back to direct llama.cpp if Agent unavailable
- Provides streaming chat interface

### Agent API

The Rust Agent provides:

- **Session Management**: Track conversation state
- **Memory Storage**: SQLite + Tantivy for persistent memory
- **LLM Client**: Calls llama.cpp for inference
- **Training Pipeline**: GRPO/LoRA fine-tuning support

### llama.cpp Server

Handles LLM inference:

- Serves GGUF quantized models
- OpenAI-compatible API endpoint
- Streaming support
- CUDA acceleration

---

## Agent Core

### The Agent Struct

The main `Agent` is defined in `src/agent/mod.rs`:

```rust
pub struct Agent {
    config: AgentConfig,
    llm_client: LlmClient,
    memory_retriever: Option<Arc<MemoryRetriever>>,
    tool_registry: Arc<ToolRegistry>,
    session_manager: SessionManager,
    reasoning_engine: ReasoningEngine,
}
```

### Creating an Agent

```rust
use agi_agent::{Agent, AgentConfig, AppConfig};

let config = AppConfig::load()?;
let agent_config = AgentConfig {
    system_prompt: "You are a helpful assistant".to_string(),
    max_context_length: 16384,
    enable_reasoning: true,
    enable_memory: true,
    enable_tools: true,
    max_tool_calls: 10,
    reasoning_depth: 3,
};

let agent = Agent::new(config, agent_config)?
    .with_memory(store, embedding_client);
```

### Chat Interaction

```rust
use agi_agent::models::{Message, Role};

// Create messages
let messages = vec![
    Message::system("You are a helpful assistant"),
    Message::user("What is Rust?"),
];

// Chat with the model
let response = agent.chat(messages).await?;
```

---

## Memory System

### Architecture

The memory system uses a two-tier approach:

```
┌─────────────────────────────────────────────────────────────┐
│                    Retrieval Namespace                        │
│  Purpose: Short-term context for conversations               │
│  Storage: SQLite + Tantivy                                  │
│  TTL: 24 hours (configurable)                              │
│  Eviction: Delete or move to training                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ On TTL expiration
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Training Namespace                        │
│  Purpose: Long-term knowledge for RL training                 │
│  Storage: SQLite (persistent)                               │
│  TTL: Never (accumulates over time)                        │
│  Usage: Training examples for GRPO                          │
└─────────────────────────────────────────────────────────────┘
```

### Memory Store

Located in `src/memory/store.rs`, implements `MemoryStore` trait:

```rust
pub trait MemoryStore: Send + Sync {
    fn store(&self, memory: &Memory) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<Memory>>;
    fn query(&self, embedding: &[f32], namespace: &str, limit: usize, min_score: f32) -> Result<Vec<SearchResult>>;
    fn update(&self, memory: &Memory) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
    fn list(&self, namespace: &str, limit: usize) -> Result<Vec<Memory>>;
    fn get_expired(&self, namespace: &str, ttl_hours: u64) -> Result<Vec<Memory>>;
}
```

### Embedding Client

Generates vector embeddings for memories:

```rust
use agi_agent::memory::embedding::EmbeddingClient;

let client = EmbeddingClient::new(EmbeddingClientConfig {
    base_url: "http://localhost:11434".to_string(),
    model: "nomic-embed-text".to_string(),
    dimensions: 768,
    batch_size: 32,
});

// Single embedding
let embedding = client.embed("Rust is a systems language").await?;
```

### Memory Retriever

Retrieves relevant memories for a query:

```rust
use agi_agent::memory::retrieval::MemoryRetriever;

let retriever = MemoryRetriever::new(store, client, RetrievalConfig {
    max_memories: 10,
    min_similarity: 0.6,
});

// Query memories
let results = retriever.retrieve("Rust programming").await?;

// Build context string
let context = retriever.build_context("Rust programming").await?;
```

### Memory Types

```rust
pub enum MemoryType {
    Fact,           // Specific facts (evict after TTL)
    Concept,        // Conceptual knowledge (move to training)
    Conversation,   // Conversation transcripts
    ToolResult,     // Results from tool execution
}
```

### Eviction Policy

Determines what happens to memories when TTL expires:

```rust
use agi_agent::memory::eviction::{MemoryEvictionPolicy, EvictionDecision};

let policy = MemoryEvictionPolicy::new(store, EvictionPolicyConfig {
    retrieval_ttl_hours: 24,
    max_retrieval_memories: 10000,
    min_quality_score: 0.3,
    auto_evict_to_training: true,
});

// Evaluate a memory
match policy.evaluate(&memory) {
    EvictionDecision::Keep => { /* stay in retrieval */ }
    EvictionDecision::EvictToTraining => { /* move to training */ }
    EvictionDecision::Delete => { /* remove entirely */ }
}
```

---

## Tool System

### Tool Trait

All tools implement the `Tool` trait:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError>;
    fn is_enabled(&self) -> bool { true }
}
```

### Available Tools

| Tool | File | Purpose |
|------|------|---------|
| `search` | `tools/search.rs` | Web search via SearXNG |
| `fetch` | (integrated) | Fetch webpage content |
| `bash` | `tools/bash.rs` | Execute shell commands |
| `read` | `tools/files.rs` | Read file contents |
| `write` | `tools/files.rs` | Write file contents |

### Tool Registry

Central management for all tools:

```rust
use agi_agent::tools::registry::ToolRegistry;

let registry = ToolRegistry::default_registry();

// Get function schemas for OpenAI
let schemas = registry.get_function_schemas();

// Execute a tool
let result = registry.execute("search", &args)?;
```

---

## Background Services

### Service Trait

All background services implement `BackgroundService`:

```rust
pub trait BackgroundService: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self) -> Result<()>;
    fn is_enabled(&self) -> bool;
}
```

### Session Review Service

Analyzes completed sessions to generate training data:

```rust
use agi_agent::services::session_review::{SessionReviewService, SessionReviewConfig};

let service = SessionReviewService::new(
    session_manager,
    memory_store,
    SessionReviewConfig {
        enabled: true,
        min_session_length: 2,
        max_sessions_per_run: 10,
        quality_threshold: 0.5,
    },
);
```

### Memory Eviction Service

Manages memory lifecycle:

```rust
use agi_agent::services::memory_eviction::{MemoryEvictionService, MemoryEvictionConfig};

let service = MemoryEvictionService::new(
    memory_store,
    eviction_policy,
    MemoryEvictionConfig {
        enabled: true,
        namespaces: vec!["retrieval".to_string()],
    },
);

// Process eviction
service.process_eviction().await?;
```

---

## Training Pipeline

### GRPO Training

Group Relative Policy Optimization implementation:

```rust
// Reward functions
pub fn format_reward(completion: &str) -> f32 {
    let mut score = 0.0;
    if completion.contains("<REASONING>") && completion.contains("</REASONING>") {
        score += 1.0;
    }
    if completion.contains("<SOLUTION>") && completion.contains("</SOLUTION>") {
        score += 1.0;
    }
    score
}
```

### Training Configuration

```toml
[training]
enabled = true
schedule = "0 2 * * *"  # 2 AM daily
model = "qwen3.5-4b"
output_path = ".agent/models"
batch_size = 4
steps = 500
learning_rate = 5e-6
lora_rank = 16
```

---

## API Reference

### Chat Completions

```bash
POST /v1/chat/completions

{
  "model": "agent",
  "messages": [
    {"role": "system", "content": "You are helpful"},
    {"role": "user", "content": "Hello"}
  ]
}
```

### Memory Endpoints

```bash
# Query memories
POST /memories/query
{"query": "rust", "limit": 10}

# Store memory
POST /memories
{"content": "...", "tags": ["concept"], "memory_type": "concept"}

# List memories
GET /memories

# Delete memory
DELETE /memories/{id}
```

### Training Endpoints

```bash
# Trigger training
POST /training/trigger

# Get status
GET /training/status

# List models
GET /training/models
```

---

## Configuration

### Full Configuration

```toml
# agent.toml

[server]
host = "0.0.0.0"
port = 8080

[model]
base_url = "http://10.10.199.146:8081"
name = "qwen3.5-4b"
embedding_model = "nomic-embed-text"
embedding_dim = 768
max_tokens = 4096
temperature = 0.7

[memory]
storage_path = "/home/administrator/.agi/memory"
retrieval_ttl_hours = 24
default_namespace = "retrieval"
min_similarity = 0.6
query_limit = 10

[training]
enabled = true
schedule = "0 2 * * *"
model = "qwen3.5-4b"
output_path = ".agent/models"
epochs = 3
batch_size = 4
learning_rate = 1e-4
lora_rank = 16
```

---

## Extending the System

### Creating a Custom Tool

```rust
use agi_agent::tools::{Tool, ToolError, ToolResult};

pub struct CalculatorTool;

impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }
    
    fn description(&self) -> &str {
        "Perform mathematical calculations"
    }
    
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        })
    }
    
    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {
        let expr = arguments["expression"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments("Missing expression".to_string()))?;
        
        // Evaluate expression
        let result = evaluate_expression(expr);
        
        Ok(ToolResult {
            tool_call_id: "calculator".to_string(),
            name: "calculator".to_string(),
            content: format!("Result: {}", result),
            success: true,
            error: None,
        })
    }
}

// Register
registry.register(CalculatorTool);
```

### Custom Agent Configuration

```rust
use agi_agent::agent::{Agent, AgentConfig};

let custom_config = AgentConfig {
    system_prompt: r#"
    You are a specialized coding assistant.
    You have access to code execution tools.
    Always explain your reasoning.
    "#.to_string(),
    max_context_length: 32768,
    enable_reasoning: true,
    reasoning_depth: 5,
    max_tool_calls: 20,
    ..Default::default()
};

let agent = Agent::new(config, custom_config)?
    .with_memory(store, client);
```

---

## Architecture Decisions

### Why Rust?

- **Performance**: Fast startup, low memory overhead
- **Safety**: Memory safety without garbage collection
- **Concurrency**: Fearless async/await
- **Ecosystem**: Strong ML and web frameworks

### Why llama.cpp?

- **GGUF Support**: Efficient quantized model serving
- **CUDA Acceleration**: GPU acceleration for inference
- **OpenAI Compatibility**: Drop-in API compatibility
- **Local Deployment**: No cloud dependency

### Why Two Memory Namespaces?

- **Retrieval**: Fast access for conversation context
- **Training**: Accumulated knowledge for RL

Separation allows:
- Different eviction policies
- Different storage optimizations
- Clear data lifecycle management

### Why GRPO Over PPO?

- Simpler implementation
- Works well with format rewards
- Good for agentic tasks

---

## Performance Considerations

### Memory System

- Embeddings cached in-memory (LRU)
- Batch embedding requests
- Async I/O throughout

### LLM Client

- Connection pooling
- Request timeouts
- Streaming for real-time responses

### Training

- LoRA for efficiency
- Gradient accumulation
- Mixed precision (where supported)

---

## Troubleshooting

### Agent Not Responding

```bash
# Check if agent is running
curl http://localhost:8080/health

# Check logs
journalctl -u agent -f
```

### Memory Not Being Retrieved

```bash
# Check memory count
curl http://localhost:8080/memories | jq '.total'

# Query with lower threshold
curl -X POST http://localhost:8080/memories/query \
  -d '{"query": "...", "min_similarity": 0.4}'
```

### Training Stuck

```bash
# Check status
curl http://localhost:8080/training/status

# Cancel and restart
pkill -f agent
./build.sh --bin agent
/tmp/target/release/agent
```
