# AGI Agent System Documentation

## Table of Contents

1. [System Overview](#system-overview)
2. [Architecture](#architecture)
3. [Agent Core](#agent-core)
4. [Memory System](#memory-system)
5. [Tool System](#tool-system)
6. [Multi-Agent Teams](#multi-agent-teams)
7. [External Knowledge](#external-knowledge)
8. [Background Services](#background-services)
9. [Training Pipeline](#training-pipeline)
10. [Online Learning](#online-learning)
11. [Curiosity Engine](#curiosity-engine)
12. [Self-Improvement](#self-improvement)
13. [Theory of Mind](#theory-of-mind)
14. [API Reference](#api-reference)
15. [Configuration](#configuration)
16. [Extending the System](#extending-the-system)

---

## System Overview

The AGI Agent is built as a collection of cooperating components enabling autonomous learning and reasoning:

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
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │  Memory Store   │  │ Curiosity Engine│  │   Theory of Mind           │ │
│  │  (Tantivy)     │  │                │  │   Modeling                 │ │
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

- **Session Management**: Track conversation state with SQLite persistence
- **Memory Storage**: SQLite + Tantivy for persistent memory with vector search
- **LLM Client**: Calls llama.cpp for inference with function calling support
- **Multi-Modal Support**: Images and audio via base64 encoding
- **Training Pipeline**: GRPO/LoRA fine-tuning support
- **Background Services**: Session review, memory eviction, curiosity, self-improvement

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
    config: Arc<RwLock<AppConfig>>,
    llm_client: Arc<RwLock<LlmClient>>,
    memory_retriever: Option<Arc<MemoryRetriever>>,
    tool_registry: Arc<ToolRegistry>,
    session_manager: SessionManager,
    reasoning_engine: ReasoningEngine,
}
```

The agent uses `Arc<RwLock<>>` wrappers to enable runtime model hot-swapping without service interruption.

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

// Multi-modal message with image
let messages = vec![
    Message::user_with_image(
        "What is in this image?",
        "https://example.com/photo.png"
    ),
];
let response = agent.chat(messages).await?;
```

### Model Hot-Swap

The agent supports runtime model switching without service interruption:

```rust
// Update model configuration at runtime
agent.update_model(ModelConfig {
    base_url: "http://localhost:11434".to_string(),
    name: "llama3:70b".to_string(),
    max_tokens: 4096,
    temperature: 0.7,
}).await?;
```

---

## Memory System

### Architecture

The memory system uses a three-tier approach:

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
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Team Namespace                          │
│  Purpose: Shared knowledge for multi-agent collaboration    │
│  Storage: SQLite (persistent)                               │
│  Usage: Inter-agent knowledge sharing                       │
└─────────────────────────────────────────────────────────────┘
```

### Memory Store

Located in `src/memory/store.rs`, implements `MemoryStore` trait with SQLite backend:

```rust
pub trait MemoryStore: Send + Sync {
    fn store(&self, memory: &Memory) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<Memory>>;
    fn query(&self, embedding: &[f32], namespace: &str, limit: usize, min_score: f32) -> Result<Vec<SearchResult>>;
    fn update(&self, memory: &Memory) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
    fn list(&self, namespace: &str, limit: usize) -> Result<Vec<Memory>>;
    fn get_expired(&self, namespace: &str, ttl_hours: u64) -> Result<Vec<Memory>>;
    fn search_by_text(&self, namespace: &str, search_text: &str, limit: usize) -> Result<Vec<QueryResult>>;
    fn search_by_text_fast(&self, namespace: &str, search_text: &str, limit: usize) -> Result<Vec<Memory>>;
}
```

### Embedding Client

Generates vector embeddings using Qwen3-Embedding server (llama.cpp):

```rust
use agi_agent::memory::embedding::EmbeddingClient;

let client = EmbeddingClient::new(EmbeddingClientConfig {
    base_url: "http://127.0.0.1:8083".to_string(),  // llama-embedding service
    model: "qwen3-embedding".to_string(),
    dimensions: 768,
    batch_size: 32,
});

// Single embedding
let embedding = client.embed("Rust is a systems language").await?;
```

**Available embedding services:**
- `llama-embedding` (port 8083): Qwen3-Embedding-0.6B-Q8_0
- `llama-rerank` (port 8084): Qwen3-Reranker-0.6B-Q8_0

### RAG (Retrieval-Augmented Generation)

RAG uses fast SQLite text search for retrieval:

```rust
// Extract key terms from query
let stop_words = ["what", "who", "where", "when", "why", "how", "is", "are", ...];
let search_term = query.split_whitespace()
    .filter(|w| w.len() > 2)
    .filter(|w| !stop_words.contains(&w.to_lowercase()))
    .next()
    .unwrap_or_default();

// Fast text search in both namespaces
let memories = memory_store.search_by_text_fast("training", search_term, 5)?;

// Inject into context
let memory_context = format!("Relevant context:\n{}",
    memories.iter().map(|m| m.content.clone()).collect::<Vec<_>>().join("\n"));
```

**Benefits:**
- Fast (< 100ms per query)
- No embedding generation at query time
- Filters stop words for better matching
- Searches both retrieval and training namespaces

### Memory Types

```rust
pub enum MemoryType {
    Fact,           // Specific facts → move to training on eviction
    Concept,        // Conceptual knowledge → already in training
    Conversation,   // Conversation transcripts → delete on eviction
    ToolResult,     // Results from tool execution → delete on eviction
}
```

### Eviction Policy

Determines what happens to memories when TTL expires. **Facts are kept, not deleted:**

```rust
use agi_agent::memory::eviction::{MemoryEvictionPolicy, EvictionDecision};

let policy = MemoryEvictionPolicy::new(store, EvictionPolicyConfig {
    retrieval_ttl_hours: 24,
    max_retrieval_memories: 10000,
    min_quality_score: 0.3,
    auto_evict_to_training: true,
});

// Evaluate a memory on TTL expiration
match policy.evaluate(&memory) {
    EvictionDecision::Keep => { /* stays in retrieval */ }
    EvictionDecision::EvictToTraining => { /* move to training (KEEP the knowledge!) */ }
    EvictionDecision::Delete => { /* remove entirely (only for conversations) */ }
}
```

**Key principle:** Facts and concepts are moved to training namespace (preserved), not deleted. Only ephemeral data (conversations, tool results) is deleted.

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
| `fetch` | `tools/fetch.rs` | Fetch webpage content |
| `fetch_image` | `tools/image.rs` | Fetch images from URLs or local files |
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

### Multi-Modal Tool: fetch_image

Fetches images for vision model analysis:

```rust
// In tool parameters
{
    "url": "https://example.com/image.png",  // OR
    "path": "/local/image.png",
    "return_base64": true,
    "metadata_only": false
}
```

Returns base64-encoded image data for vision model processing.

---

## Multi-Agent Teams

The agent supports multi-agent collaboration with shared memory and role-based task delegation.

### Agent Roles

```rust
pub enum AgentRole {
    Assistant,  // General help
    Coder,      // Code tasks (handles "code", "function", "debug")
    Researcher, // Research tasks (handles "research", "find", "search")
    Writer,     // Writing tasks
    Analyst,    // Analysis tasks
    Custom(String), // Custom role
}
```

### Creating a Team

```rust
use agi_agent::agent::team::{AgentTeam, AgentRole};

let team = AgentTeam::with_default_roles(
    config.clone(),
    Arc::new(RwLock::new(memory_store)),
);

// Process query with automatic agent selection
let response = team.process("Write a Python function").await?;
```

### Team Features

- **Automatic Agent Selection**: Keywords determine which agent handles a query
- **Shared Memory**: Team namespace for inter-agent knowledge sharing
- **Response Synthesis**: Combines multiple agent outputs into cohesive response

```rust
// Store shared knowledge
team.store_shared_memory("Project deadline is Friday").await?;

// Get shared memories
let memories = team.get_shared_memories().await?;
```

---

## External Knowledge

Query external knowledge sources for up-to-date information.

### Knowledge Sources

| Source | Description | Endpoint |
|--------|-------------|----------|
| Wikipedia | Encyclopedia articles | `/knowledge/wikipedia/{title}` |
| ArXiv | Academic papers | `/knowledge/arxiv/{id}` |
| Web Fetcher | General web content | `/knowledge/fetch` |

### Wikipedia Client

```rust
use agi_agent::knowledge::wikipedia::WikipediaClient;

let client = WikipediaClient::new("en");
let article = client.get_article("Rust_(programming_language)").await?;
let results = client.search("Rust programming").await?;
```

### ArXiv Client

```rust
use agi_agent::knowledge::arxiv::ArxivClient;

let client = ArxivClient::new();
let paper = client.get_paper("2303.08774").await?;
let results = client.search("machine learning transformers").await?;
```

### Web Fetcher

```rust
use agi_agent::knowledge::fetch::WebFetcher;

let fetcher = WebFetcher::new(30); // 30 second timeout
let content = fetcher.fetch("https://example.com").await?;
```

### Knowledge Entry

Results are returned as structured `KnowledgeEntry`:

```rust
pub struct KnowledgeEntry {
    pub source: KnowledgeSource,
    pub title: String,
    pub content: String,
    pub url: Option<String>,
    pub relevance_score: f32,
    pub metadata: HashMap<String, Value>,
}
```

Entries can be converted to memory for storage:

```rust
let memory: Memory = knowledge_entry.to_memory("training");
memory_store.store(&memory)?;
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

Analyzes completed sessions to generate training data. Session review is automatically triggered when a session ends via the `/sessions/{id}/end` endpoint.

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
        use_llm_enhancement: true,  // LLM-enhanced training data generation
        llm_base_url: None,
        llm_model: None,
    },
);

// Analyze a session
let result = service.review_session(session_id, &messages).await?;
```

The service:
1. Extracts facts and concepts from conversations
2. Generates training examples from good conversations (LLM-enhanced)
3. Identifies topics for further research
4. Moves useful memories to training namespace
5. Automatically triggered on session end

### Memory Eviction Service

Manages memory lifecycle with TTL-based eviction. Can run on a schedule via the Scheduler service.

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

// Get eviction statistics
let stats = service.get_stats().await;
```

### Search Learning Service

Researches topics using SearXNG when knowledge gaps are detected. Results are automatically converted to training examples and added to batch training.

```rust
use agi_agent::services::search_learning::SearchLearningService;

let service = SearchLearningService::new(
    search_client,
    memory_store,
    embedding_client,
);

// Wire to batch training service
service.set_batch_training_service(batch_training_service.clone());

// Research a topic (generates training examples automatically)
let result = service.learn("Rust ownership model").await?;

// Check accumulated training examples
let count = service.get_training_examples_count().await;
```

---

### Scheduler Service

Cron-based scheduler for automated background tasks:

```rust
use agi_agent::services::scheduler::{SchedulerService, SchedulerConfig};

let scheduler = SchedulerService::with_services(
    SchedulerConfig {
        batch_training_enabled: true,
        batch_training_schedule: "0 2 * * *".to_string(), // 2 AM daily
        memory_eviction_enabled: true,
        memory_eviction_schedule: "0 0 * * *".to_string(), // Midnight
        session_review_enabled: false, // Triggered on session end
    },
    batch_training_service,
    memory_eviction_service,
    session_review_service,
);

// Start the scheduler
scheduler.start().await?;

// Get statistics
let stats = scheduler.get_stats().await;

// Manually trigger batch training
scheduler.trigger_batch_training().await?;

// Stop the scheduler
scheduler.stop().await?;
```

## Training Pipeline

### GRPO Training

Group Relative Policy Optimization implementation:

```rust
use agi_agent::training::grpo;

// Format reward for structured output
let format_score = grpo::format_reward(completion);

// Helpfulness reward
let helpful_score = grpo::helpfulness_reward(completion, reference);

// Combined reward
let total_reward = grpo::combined_reward(completion, reference, 0.5);
```

### Reward Functions

```rust
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

pub fn helpfulness_reward(completion: &str, reference: &str) -> f32 {
    // Cosine similarity between completion and reference
    cosine_similarity(completion, reference)
}
```

### Batch Training Service

Integrates training module with API. Training examples are automatically persisted to disk (`~/.agi/training/examples.jsonl`).

```rust
use agi_agent::services::batch_training::{BatchTrainingService, BatchTrainingConfig};

let service = BatchTrainingService::new(config);

// Add training example (auto-persisted to disk)
service.add_example(training_example).await?;

// Collect from memory
service.collect_from_memory().await?;

// Run training (clears examples after success)
service.train().await?;

// Export as JSONL
let jsonl = service.export_jsonl().await?;

// Clear all accumulated examples
service.clear().await;
```

### Model Registry

Manages trained model versions:

```rust
use agi_agent::training::registry::ModelRegistry;

let registry = ModelRegistry::new(PathBuf::from(".agent/models"));

// Save model
registry.save_model(model_id, adapter_path).await?;

// List models
let models = registry.list_models().await?;

// Set current model
registry.set_current_model(model_id).await?;
```

### Training Configuration

```toml
[training]
enabled = true
schedule = "0 2 * * *"  # 2 AM daily
model = "qwen3.5-4b"
output_path = ".agent/models"
epochs = 3
batch_size = 4
steps = 500
learning_rate = 1e-4
lora_rank = 16
```

---

## Online Learning

Continuous reinforcement learning from interactions using prioritized experience replay.

### Experience Replay Buffer

```rust
use agi_agent::services::online_learning::{OnlineLearningService, Experience};

let service = OnlineLearningService::new(config);

// Add experience
let experience = Experience {
    prompt: "What is Rust?".to_string(),
    completion: "Rust is a systems programming language...".to_string(),
    reward: 0.8,
    priority: 0.7,
};
service.add_experience(experience).await?;

// Perform learning update
let update = service.learn(batch_size).await?;
```

### Priority Calculation

Priority = (reward * 0.6) + (quality * 0.4) + novelty_bonus

- High-priority experiences (priority > 0.7) are never auto-evicted
- Adaptive learning rate adjusts based on recent performance
- Replay ratio: 30% from buffer, 70% fresh examples

### Configuration

```toml
[online_learning]
batch_size = 16
max_buffer_size = 1000
replay_ratio = 0.3
learning_rate = 1e-5
min_buffer_for_training = 50
adaptive_learning_rate = true
update_interval_seconds = 300
```

---

## Curiosity Engine

Autonomous exploration of topics the agent doesn't understand well.

### Knowledge Gap Detection

```rust
use agi_agent::services::curiosity::{CuriosityEngine, KnowledgeGap};

let engine = CuriosityEngine::new(config);

// Detect gaps in conversation
let gaps = engine.detect_gaps(&messages).await?;

for gap in gaps {
    println!("Gap: {} (curiosity: {})", gap.topic, gap.curiosity_score);
}
```

### Gap Types

```rust
pub enum KnowledgeGapReason {
    UserQuestion,      // User asked about unknown topic
    AgentUncertainty,  // Agent expressed low confidence
    FailedSearch,      // Search returned no results
    Contradiction,     // Conflicting information detected
    TopicMention,      // Topic mentioned but not explained
    NovelConcept,      // New concept detected
}
```

### Exploration Queue

```rust
// Add gap to exploration queue
engine.queue_gap(gap).await?;

// Process exploration (wired to batch training automatically)
let result = engine.explore(gap_id).await?;

// Process all pending gaps
engine.process_queue(max_explorations).await?;

// Wire to batch training service
engine.wire_to_batch_training(batch_training_service).await?;
```

Exploration uses Wikipedia and ArXiv to learn about gaps. Discovered knowledge is automatically stored in training memory and converted to training examples via the wired batch training service.

### Configuration

```toml
[curiosity]
enabled = true
max_gaps = 50
curiosity_threshold = 0.5
exploration_depth = 2
```

---

## Self-Improvement

The self-improvement engine analyzes code patterns and generates improvements to the agent.

### Code Pattern Detection

```rust
use agi_agent::services::self_improve::{SelfImproveEngine, CodePatternType};

let engine = SelfImproveEngine::new(config);

// Analyze search results for patterns
let patterns = engine.detect_code_patterns(&search_results)?;

for pattern in patterns {
    println!("Found: {:?} - {}", pattern.pattern_type, pattern.description);
}
```

### Pattern Types

```rust
pub enum CodePatternType {
    BestPractice,
    Performance,
    ErrorHandling,
    Async,
    Memory,
    ApiDesign,
    Testing,
    Security,
    Refactoring,
    MissingFeature,
}
```

### Generating Improvements

```rust
// Generate improvement suggestions
let suggestions = engine.generate_improvement_suggestions(&patterns)?;

for suggestion in suggestions {
    // Apply improvement
    engine.apply_code_improvement(&suggestion).await?;
}
```

### Improvement Lifecycle

```
Pending → Generated → Tested → Approved → Applied
                               ↓
                           Rejected
                               ↓
                          RolledBack
```

### Configuration

```toml
[self_improve]
enabled = true
code_patterns_dir = ".agent/patterns"
auto_apply = false
```

---

## Theory of Mind

Models user mental state for personalized responses.

### User Mental State

```rust
use agi_agent::services::theory_of_mind::{
    TheoryOfMindEngine, UserMentalState, Belief, Intention, EmotionalState
};

let engine = TheoryOfMindEngine::new(config);

// Update user model from conversation
engine.update_user_model(user_id, &messages).await?;

// Get user mental state
let state = engine.get_user_state(user_id).await?;

println!("Beliefs: {:?}", state.beliefs);
println!("Intentions: {:?}", state.intentions);
println!("Emotional state: {:?}", state.emotional);
```

### Belief Tracking

```rust
pub struct Belief {
    pub content: String,
    pub confidence: f32,
    pub source: String,
    pub accuracy: f32,
    pub updated_at: DateTime<Utc>,
}
```

### Intention Recognition

```rust
pub enum IntentionType {
    Learn,              // User wants to learn something
    TaskCompletion,     // User wants help completing a task
    InformationSeeking, // User seeking information
    ProblemSolving,     // User solving a problem
    CasualChat,         // Casual conversation
    Troubleshooting,    // User troubleshooting an issue
}
```

### Response Recommendations

```rust
// Get response recommendations based on user state
let analysis = engine.analyze_user(user_id).await?;

println!("Response style: {:?}", analysis.response_style);
println!("Tone adjustment: {:?}", analysis.tone_adjustment);
println!("Explanation depth: {:?}", analysis.explanation_depth);
```

### Trust Modeling

```rust
// Update trust level based on interactions
engine.update_trust(user_id, delta).await?;

// Check trust level
let trust = engine.get_trust_level(user_id).await?;
```

### Configuration

```toml
[theory_of_mind]
enabled = true
max_history = 100
trust_decay = 0.01
emotion_weights = { positive = 1.0, negative = 1.5, neutral = 0.5 }
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
{"query": "rust", "limit": 10, "namespace": "retrieval"}

# Store memory
POST /memories
{"content": "...", "tags": ["concept"], "memory_type": "concept"}

# List memories
GET /memories?namespace=training&limit=100

# Delete memory
DELETE /memories/{id}
```

### Training Endpoints

```bash
# Trigger training
POST /training/trigger

# Get status
GET /training/status

# Batch training
GET  /training/batch/status
POST /training/batch/collect
POST /training/batch/add
GET  /training/batch/stats
POST /training/batch/run
GET  /training/batch/export
POST /training/batch/clear
POST /training/batch/filter

# Model registry
GET  /training/models/list
POST /training/models/current

# Scheduler
GET  /scheduler/stats
POST /scheduler/trigger
```

### Session Endpoints

```bash
# List sessions
GET /sessions

# Create session
POST /sessions
{"user_id": "user-123"}

# Get session
GET /sessions/{id}

# End session (triggers review)
POST /sessions/{id}/end

# Delete session
DELETE /sessions/{id}
```

### Curiosity Endpoints

```bash
GET  /curiosity/stats
POST /curiosity/detect
GET  /curiosity/gaps
GET  /curiosity/gaps/pending
POST /curiosity/explore
POST /curiosity/process
POST /curiosity/dismiss
```

### Online Learning Endpoints

```bash
GET  /learning/stats
GET  /learning/buffer
POST /learning/learn
GET  /learning/concepts
POST /learning/example
POST /learning/session
POST /learning/prune
```

### Self-Improvement Endpoints

```bash
GET  /self-improve/stats
POST /self-improve/analyze
GET  /self-improve/improvements
POST /self-improve/apply
POST /self-improve/reject
POST /self-improve/rollback
GET  /self-improve/prompt
POST /self-improve/prompt
```

### Theory of Mind Endpoints

```bash
GET  /tom/stats
POST /tom/user
GET  /tom/user?user_id=...
GET  /tom/users
POST /tom/analyze
GET  /tom/history?user_id=...
POST /tom/clear
POST /tom/trust
POST /tom/intention
```

### Knowledge Endpoints

```bash
POST /knowledge/search
GET  /knowledge/wikipedia/{title}
GET  /knowledge/arxiv/{id}
POST /knowledge/fetch
GET  /knowledge/sources
```

### Model Endpoints

```bash
GET  /model/status
POST /model/validate
POST /model/update
GET  /model/available
```

### Learned Concepts

```bash
GET  /concepts
POST /concepts/search
{"query": "rust", "limit": 10}
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
base_url = "http://10.10.199.146:8081"     # llama.cpp server
name = "qwen3.5-4b"
embedding_model = "qwen3-embedding"         # Qwen3 embedding model name
embedding_base_url = "http://127.0.0.1:8083" # Embedding server
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
schedule = "0 2 * * *"  # 2 AM daily
model = "qwen3.5-4b"
output_path = ".agent/models"
epochs = 3
batch_size = 4
learning_rate = 1e-4
lora_rank = 16

[scheduler]
enabled = true
batch_training_enabled = true
batch_training_schedule = "0 2 * * *"  # 2 AM daily
memory_eviction_enabled = true
memory_eviction_schedule = "0 0 * * *"  # Midnight daily

[search]
instance = "https://search.butler.ooo"  # SearXNG
timeout = 30

[online_learning]
batch_size = 16
max_buffer_size = 1000
replay_ratio = 0.3
learning_rate = 1e-5
min_buffer_for_training = 50
adaptive_learning_rate = true
update_interval_seconds = 300

[curiosity]
enabled = true
max_gaps = 50
curiosity_threshold = 0.5
exploration_depth = 2

[theory_of_mind]
enabled = true
max_history = 100
trust_decay = 0.01
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

// Register in registry
registry.register(CalculatorTool);
```

### Creating a Custom Background Service

```rust
use agi_agent::services::BackgroundService;
use tokio::time::{interval, Duration};

pub struct MyCustomService;

#[async_trait::async_trait]
impl BackgroundService for MyCustomService {
    fn name(&self) -> &'static str {
        "my_custom_service"
    }
    
    async fn run(&self) -> Result<()> {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            self.do_work().await?;
        }
    }
    
    fn is_enabled(&self) -> bool {
        true
    }
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
    max_context_length: 32768,
    enable_reasoning: true,
    reasoning_depth: 5,
    max_tool_calls: 20,
    ..Default::default()
};

let agent = Agent::new(config, custom_config)?
    .with_memory(store, client);
```

### Adding a Knowledge Source

```rust
use agi_agent::knowledge::{KnowledgeEntry, KnowledgeSource};

pub struct MyKnowledgeSource;

impl MyKnowledgeSource {
    pub async fn query(&self, query: &str) -> Result<Vec<KnowledgeEntry>> {
        // Fetch and parse data
        let entries = vec![KnowledgeEntry {
            source: KnowledgeSource::Custom("my_source".to_string()),
            title: "Result".to_string(),
            content: "...".to_string(),
            url: None,
            relevance_score: 0.9,
            metadata: HashMap::new(),
        }];
        Ok(entries)
    }
}
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
- **Embedding Server**: Built-in support for Qwen3-Embedding models

### Why SQLite + Text Search for RAG?

- **Speed**: SQLite LIKE queries are fast (< 10ms)
- **Simplicity**: No embedding generation at query time
- **Reliability**: ACID transactions, proven technology
- **Compatibility**: Works with any memory content

### Why Three Memory Namespaces?

- **Retrieval**: Short-term context, expires after TTL
- **Training**: Long-term knowledge, never deleted
- **Team**: Shared knowledge for multi-agent

Separation allows:
- Different eviction policies
- Different storage optimizations
- Clear data lifecycle management

### Why GRPO Over PPO?

- Simpler implementation
- Works well with format rewards
- Good for agentic tasks

### Why Online + Batch Learning?

- **Online**: Immediate learning from interactions
- **Batch**: Comprehensive training on accumulated examples
- Combined approach balances responsiveness with thoroughness

### Why Curiosity-Driven Exploration?

- Autonomous learning without human intervention
- Identifies knowledge gaps proactively
- Expands agent's knowledge base continuously

---

## Performance Considerations

### Memory System

- SQLite text search for RAG (< 10ms per query)
- Stop word filtering for better relevance
- Searches both retrieval and training namespaces
- Optional: Qwen3-Embedding for vector search (port 8083)

### LLM Client

- Connection pooling
- Request timeouts
- Streaming for real-time responses
- Runtime model hot-swap via symlink

### Training

- LoRA for efficiency (Qwen3.5-4B fine-tuned)
- unsloth for fast training
- Symlink versioning for model deployment
- Automatic systemd restart after training

### Multi-Modal

- Base64 encoding for images/audio
- Lazy loading for large files
- Metadata-only option for quick analysis
