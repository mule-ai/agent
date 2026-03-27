# AGI Agent System Documentation

A comprehensive guide to the AGI Agent system architecture, components, and extension points.

## Table of Contents

1. [System Overview](#system-overview)
2. [Agent Core](#agent-core)
3. [Memory System](#memory-system)
4. [Tool System](#tool-system)
5. [Background Services](#background-services)
6. [Training Pipeline](#training-pipeline)
7. [API Reference](#api-reference)
8. [Configuration](#configuration)
9. [Extending the System](#extending-the-system)

---

## System Overview

The AGI Agent is built as a collection of cooperating components:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           User Applications                                    │
│                    (Chat, CLI, Custom Integrations)                           │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          OpenAI-Compatible API                                │
│                       /v1/chat/completions, /v1/models                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                             Agent Core                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ Session Manager │  │ Reasoning Engine│  │     LLM Client              │ │
│  │ (State)        │  │ (Thinking)     │  │ (Ollama/OpenAI)            │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                    │                       │
                    ▼                       ▼
        ┌───────────────────┐     ┌───────────────────────┐
        │   Memory System   │     │     Tool System       │
        │ (Tantivy/SQLite) │     │ (Search, Bash, Files) │
        └───────────────────┘     └───────────────────────┘
                    │
                    ▼
        ┌───────────────────────────────────────────────┐
        │              Background Services                  │
        │  (Session Review, Search Learning, Eviction)     │
        └───────────────────────────────────────────────┘
                    │
                    ▼
        ┌───────────────────────────────────────────────┐
        │             Training Pipeline                     │
        │           (GRPO / LoRA / Candle)                │
        └───────────────────────────────────────────────┘
```

---

## Agent Core

### The Agent Struct

The main `Agent` is defined in `src/agent/mod.rs`:

```rust
pub struct Agent {
    config: AgentConfig,
    llm_client: LLMClient,
    memory_retriever: Option<Arc<MemoryRetriever>>,
    tool_registry: Arc<ToolRegistry>,
    session_manager: Arc<SessionManager>,
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

// Non-streaming
let response = agent.chat(messages, false).await?;

// Streaming
let response = agent.chat(messages, true).await?;
```

---

## Memory System

### Architecture

The memory system uses a two-tier approach:

```
┌─────────────────────────────────────────────────────────────┐
│                    Retrieval Namespace                        │
│  Purpose: Short-term context for conversations               │
│  Storage: SQLite + Tantivy + In-memory cache               │
│  TTL: 24 hours (configurable)                              │
│  Eviction: Delete or move to training                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ On TTL expiration
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Training Namespace                        │
│  Purpose: Long-term knowledge for RL training               │
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
    fn query_text(&self, query: &str, namespace: &str, limit: usize) -> Result<Vec<SearchResult>>;
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
    api_key: None,
});

// Single embedding
let embedding = client.embed("Rust is a systems language").await?;

// Batch embeddings
let embeddings = client.embed_batch(&texts).await?;
```

### Memory Retriever

Retrieves relevant memories for a query:

```rust
use agi_agent::memory::retrieval::MemoryRetriever;

let retriever = MemoryRetriever::new(store, client, RetrievalConfig {
    max_memories: 10,
    min_similarity: 0.6,
    ..Default::default()
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
    LearnedPattern, // Learned patterns/behaviors
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
    EvictionDecision::MakePersistent => { /* never evict */ }
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

### Search Tool

```rust
use agi_agent::tools::search::{SearchTool, SearchConfig};

let search = SearchTool::new(SearchConfig {
    instance: "https://search.butler.ooo".to_string(),
    timeout: 30,
    num_results: 10,
});

// Execute
let args = serde_json::json!({
    "query": "rust programming",
    "num_results": 5
});
let result = search.execute(&args)?;
```

### Bash Tool

```rust
use agi_agent::tools::bash::BashTool;

let bash = BashTool::new()
    .with_working_dir("/home/user")
    .with_timeout(60);

let args = serde_json::json!({
    "command": "ls -la"
});
let result = bash.execute(&args)?;
```

### Tool Registry

Central management for all tools:

```rust
use agi_agent::tools::registry::ToolRegistry;

let registry = ToolRegistry::default_registry();

// Register custom tool
registry.register(MyCustomTool::new());

// Get function schemas for OpenAI
let schemas = registry.get_function_schemas();

// Execute a tool
let result = registry.execute("bash", &args)?;
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
    embedding_client,
    SessionReviewConfig {
        enabled: true,
        min_session_length: 2,
        max_sessions_per_run: 10,
        quality_threshold: 0.5,
    },
);

// Process completed sessions
let result = service.process_completed_sessions().await?;
```

### Search Learning Service

Expands knowledge by researching topics:

```rust
use agi_agent::services::search_learning::{SearchLearningService, SearchLearningConfig};

let service = SearchLearningService::new(
    memory_store,
    embedding_client,
    SearchLearningConfig::default(),
);

// Queue topics for research
service.queue_topic("How does quantum entanglement work?".to_string());

// Process learning queue
let result = service.process_queue().await?;
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
        create_training_examples: true,
    },
);

// Process eviction
let result = service.process_eviction().await?;

// Get statistics
let stats = service.get_stats().await?;
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

pub fn correctness_reward(completion: &str, expected: &str) -> f32 {
    // Extract answer and compare
    // Return 2.0 for correct, 0.0 for incorrect
}
```

### Training Configuration

```toml
[training]
enabled = true
schedule = "0 2 * * *"  # 2 AM daily
model = "qwen3:8b"
output_path = ".agent/models"
batch_size = 4
steps = 500
learning_rate = 5e-6
lora_rank = 16
```

### Running Training

```bash
# Via API
curl -X POST http://localhost:8080/training/trigger

# Via CLI
./target/release/training --steps 500 --lr 5e-6

# Check status
curl http://localhost:8080/training/status
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
  ],
  "stream": false
}
```

Response:
```json
{
  "id": "chatcmpl-xxx",
  "choices": [{
    "message": {
      "role": "assistant",
      "content": "Hello! How can I help?"
    }
  }]
}
```

### Memory Endpoints

```bash
# Query memories
POST /memories/query
{"query": "rust", "namespace": "retrieval", "limit": 10}

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
workers = 4

[model]
base_url = "http://localhost:11434"
name = "qwen3:8b"
embedding_model = "nomic-embed-text"
embedding_dim = 768
max_tokens = 4096
temperature = 0.7

[memory]
storage_path = ".agent/memory"
retrieval_ttl_hours = 24
default_namespace = "retrieval"
min_similarity = 0.6
query_limit = 10

[search]
instance = "https://search.butler.ooo"
timeout = 30
results = 10

[training]
enabled = true
schedule = "0 2 * * *"
model = "qwen3:8b"
output_path = ".agent/models"
batch_size = 4
steps = 500
learning_rate = 5e-6
lora_rank = 16

[tools]
search_enabled = true
bash_enabled = true
file_tools_enabled = true
```

### Environment Variables

```bash
# Override via environment
export AGENT_MODEL_BASE_URL="http://custom-llm:11434"
export AGENT_MEMORY_PATH="/data/memory"
export TRAINING_STEPS=1000
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
        
        // Evaluate (simplified)
        let result = evaluate_expression(expr);
        
        Ok(ToolResult {
            tool_call_id: "calculator".to_string(),
            name: "calculator".to_string(),
            content: format!("Result: {}", result),
            success: true,
            error: None,
            metadata: None,
        })
    }
}

// Register
registry.register(CalculatorTool);
```

### Creating a Custom Background Service

```rust
use agi_agent::services::BackgroundService;

pub struct MyCustomService {
    config: MyConfig,
}

#[async_trait::async_trait]
impl BackgroundService for MyCustomService {
    fn name(&self) -> &'static str {
        "my_custom_service"
    }
    
    async fn run(&self) -> anyhow::Result<()> {
        // Custom logic here
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}
```

### Creating a Custom Memory Backend

```rust
use agi_agent::memory::store::MemoryStore;
use agi_agent::models::{Memory, SearchResult};

pub struct RedisMemoryStore {
    client: redis::Client,
}

impl MemoryStore for RedisMemoryStore {
    fn store(&self, memory: &Memory) -> anyhow::Result<()> {
        // Custom storage logic
        Ok(())
    }
    
    fn query(&self, embedding: &[f32], namespace: &str, limit: usize, min_score: f32) 
        -> anyhow::Result<Vec<SearchResult>> {
        // Custom query logic
        Ok(vec![])
    }
    
    // ... implement remaining methods
}
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
    max_context_length: 32768,  // Larger context
    enable_reasoning: true,
    reasoning_depth: 5,  // Deeper reasoning
    max_tool_calls: 20,  // More tools allowed
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

### Why LoRA?

- Parameter-efficient fine-tuning
- Fast training
- Easy model swapping

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

## Security Considerations

### Bash Tool

- Command allowlist/denylist
- Working directory restrictions
- Execution timeout
- Output size limits

### File Tools

- Path restrictions (no /etc, /sys, /proc)
- Directory traversal prevention
- Permission checks

### Memory Access

- Namespace isolation
- Future: encryption at rest

---

## Future Enhancements

### Phase 2

- Multi-modal support (images, audio)
- Persistent user sessions
- Team of agents with shared memory
- External knowledge base integration

### Phase 3

- Continuous learning (online RL)
- Curiosity-driven exploration
- Self-improvement through code generation
- Theory of mind modeling

---

## Troubleshooting

### Agent Not Responding

```bash
# Check health
curl http://localhost:8080/health

# Check logs
journalctl -u agi-agent -f
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
pkill -f training
./target/release/training
```
