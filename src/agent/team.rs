//! Multi-agent team coordination
//! 
//! Implements team of agents with shared memory as specified in SPEC.md Phase 2:
//! - Multiple agents with different specialties
//! - Shared memory for inter-agent communication
//! - Task delegation based on agent capabilities
//! - Collaborative problem solving

use crate::agent::{Agent, AgentConfig, AgentError};
use crate::config::AppConfig;
use crate::memory::{EmbeddingClient, SqliteMemoryStore};
use crate::models::{Memory, MemoryType, Message};
use crate::tools::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use itertools::Itertools;

/// Agent role/specialty in a team
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentRole {
    /// General purpose assistant
    #[allow(dead_code)]
    Assistant,
    /// Focused on coding and technical tasks
    #[allow(dead_code)]
    Coder,
    /// Focused on research and information gathering
    #[allow(dead_code)]
    Researcher,
    /// Focused on writing and content creation
    #[allow(dead_code)]
    Writer,
    /// Focused on analysis and reasoning
    #[allow(dead_code)]
    Analyst,
    /// Custom role
    Custom(String),
}

impl AgentRole {
    /// Get the system prompt suffix for this role
    #[allow(dead_code)]
    pub fn system_prompt_suffix(&self) -> String {
        match self {
            AgentRole::Assistant => String::new(),
            AgentRole::Coder => "\n\nYou are a coding specialist. Focus on writing clean, efficient code. Explain technical concepts clearly.".to_string(),
            AgentRole::Researcher => "\n\nYou are a research specialist. Focus on finding accurate information, citing sources, and thorough investigation.".to_string(),
            AgentRole::Writer => "\n\nYou are a writing specialist. Focus on clear, engaging prose. Pay attention to grammar and style.".to_string(),
            AgentRole::Analyst => "\n\nYou are an analysis specialist. Focus on breaking down problems systematically and evaluating evidence.".to_string(),
            AgentRole::Custom(name) => format!("\n\nYou are a {} specialist.", name),
        }
    }
    
    /// Get keywords that indicate this role should be consulted
    #[allow(dead_code)]
    pub fn keywords(&self) -> Vec<&'static str> {
        match self {
            AgentRole::Assistant => vec![],
            AgentRole::Coder => vec!["code", "programming", "function", "bug", "debug", "api", "algorithm", "software", "develop"],
            AgentRole::Researcher => vec!["research", "find", "search", "information", "study", "investigate", "learn about", "what is"],
            AgentRole::Writer => vec!["write", "article", "blog", "document", "story", "edit", "content", "text"],
            AgentRole::Analyst => vec!["analyze", "compare", "evaluate", "assess", "think", "reason", "explain why", "break down"],
            AgentRole::Custom(_) => vec![],
        }
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Assistant => write!(f, "Assistant"),
            AgentRole::Coder => write!(f, "Coder"),
            AgentRole::Researcher => write!(f, "Researcher"),
            AgentRole::Writer => write!(f, "Writer"),
            AgentRole::Analyst => write!(f, "Analyst"),
            AgentRole::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// A team member with a role and shared access to memory
#[allow(dead_code)]
pub struct TeamAgent {
    pub id: String,
    pub role: AgentRole,
    pub name: String,
    config: AppConfig,
    agent_config: AgentConfig,
    memory_store: Arc<SqliteMemoryStore>,
    embedding_client: Arc<EmbeddingClient>,
    tool_registry: Arc<ToolRegistry>,
}

impl TeamAgent {
    /// Create a new team agent
    pub fn new(
        name: String,
        role: AgentRole,
        config: AppConfig,
        base_system_prompt: &str,
        memory_store: Arc<SqliteMemoryStore>,
        embedding_client: Arc<EmbeddingClient>,
        tool_registry: Arc<ToolRegistry>,
    ) -> anyhow::Result<Self> {
        let system_prompt = format!("{}{}", base_system_prompt, role.system_prompt_suffix());
        
        let agent_config = AgentConfig {
            system_prompt,
            ..Default::default()
        };

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            role,
            name,
            config,
            agent_config,
            memory_store,
            embedding_client,
            tool_registry,
        })
    }

    /// Process a query and return a response
    pub async fn process(&self, query: &str) -> Result<TeamAgentResponse, AgentError> {
        let messages = vec![
            Message::user(query.to_string()),
        ];
        
        let agent = Agent::new(
            self.config.clone(),
            self.agent_config.clone(),
            self.memory_store.clone(),
            self.embedding_client.clone(),
            self.tool_registry.clone(),
        ).map_err(|e| AgentError::SessionError(e.to_string()))?;

        let response = agent.chat(messages).await?;
        
        Ok(TeamAgentResponse {
            agent_id: self.id.clone(),
            agent_name: self.name.clone(),
            agent_role: self.role.clone(),
            content: response.content,
            reasoning: response.reasoning,
        })
    }

    /// Check if this agent should handle a query based on keywords
    #[allow(dead_code)]
    pub fn should_handle(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.role.keywords().iter().any(|kw| query_lower.contains(kw))
    }
}

/// Response from a team agent
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TeamAgentResponse {
    pub agent_id: String,
    pub agent_name: String,
    pub agent_role: AgentRole,
    pub content: String,
    pub reasoning: Option<String>,
}

/// Shared context for team collaboration
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SharedContext {
    pub contributions: Vec<TeamAgentResponse>,
    pub shared_memories: Vec<Memory>,
    pub final_synthesis: Option<String>,
}

impl SharedContext {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn add_contribution(&mut self, response: TeamAgentResponse) {
        self.contributions.push(response);
    }

    #[allow(dead_code)]
    pub fn synthesis_prompt(&self, original_query: &str) -> String {
        let contributions = self.contributions.iter()
            .map(|c| format!(
                "### {} ({})\n{}\n",
                c.agent_name,
                c.agent_role,
                c.content
            ))
            .join("\n");

        format!(
            "Original query: {}\n\n\
            Team contributions:\n{}\n\n\
            Synthesize these contributions into a comprehensive response. \
            Incorporate the key insights from each specialist while maintaining coherence.",
            original_query,
            contributions
        )
    }
}

/// Team of agents with shared memory
#[allow(dead_code)]
pub struct AgentTeam {
    pub id: String,
    pub name: String,
    agents: HashMap<String, Arc<TeamAgent>>,
    memory_store: Arc<SqliteMemoryStore>,
    embedding_client: Arc<EmbeddingClient>,
    config: AppConfig,
    use_synthesis: bool,
    context: Arc<RwLock<SharedContext>>,
}

impl AgentTeam {
    /// Create a new agent team
    pub fn new(
        name: String,
        config: AppConfig,
        memory_store: Arc<SqliteMemoryStore>,
        embedding_client: Arc<EmbeddingClient>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name,
            agents: HashMap::new(),
            memory_store,
            embedding_client,
            config,
            use_synthesis: true,
            context: Arc::new(RwLock::new(SharedContext::new())),
        })
    }

    /// Add an agent to the team
    pub fn add_agent(&mut self, agent: TeamAgent) -> String {
        let id = agent.id.clone();
        self.agents.insert(id.clone(), Arc::new(agent));
        id
    }

    /// Create a default team with common roles
    pub fn with_default_roles(
        name: String,
        config: AppConfig,
        memory_store: Arc<SqliteMemoryStore>,
        embedding_client: Arc<EmbeddingClient>,
        tool_registry: Arc<ToolRegistry>,
        base_system_prompt: &str,
    ) -> anyhow::Result<Self> {
        let mut team = Self::new(name, config.clone(), memory_store.clone(), embedding_client.clone())?;

        // Add default agents
        let default_agents = vec![
            ("Assistant", AgentRole::Assistant),
            ("Coder", AgentRole::Coder),
            ("Researcher", AgentRole::Researcher),
            ("Writer", AgentRole::Writer),
            ("Analyst", AgentRole::Analyst),
        ];

        for (name, role) in default_agents {
            let agent = TeamAgent::new(
                name.to_string(),
                role,
                config.clone(),
                base_system_prompt,
                memory_store.clone(),
                embedding_client.clone(),
                tool_registry.clone(),
            )?;
            team.add_agent(agent);
        }

        Ok(team)
    }

    /// Set whether to synthesize responses from multiple agents
    pub fn with_synthesis(mut self, enabled: bool) -> Self {
        self.use_synthesis = enabled;
        self
    }

    /// Find agents that should handle a query
    fn find_agents_for_query(&self, query: &str) -> Vec<Arc<TeamAgent>> {
        let mut candidates: Vec<Arc<TeamAgent>> = self.agents.values().cloned().collect();
        
        // Sort by relevance (agents with matching keywords first)
        candidates.sort_by(|a, b| {
            let a_matches = a.should_handle(query);
            let b_matches = b.should_handle(query);
            
            match (a_matches, b_matches) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        // Always include at least the assistant
        if !candidates.iter().any(|a| a.role == AgentRole::Assistant) {
            if let Some(assistant) = self.agents.values().find(|a| a.role == AgentRole::Assistant) {
                candidates.insert(0, assistant.clone());
            }
        }

        // Limit to relevant agents (or at most 3 for synthesis)
        if candidates.iter().any(|a| a.should_handle(query)) {
            candidates.retain(|a| a.should_handle(query) || a.role == AgentRole::Assistant);
        }
        
        candidates.truncate(3);
        candidates
    }

    /// Process a query with the team
    pub async fn process(&self, query: &str) -> Result<TeamResponse, AgentError> {
        // Reset context
        {
            let mut ctx = self.context.write().await;
            *ctx = SharedContext::new();
        }

        // Find relevant agents
        let agents = self.find_agents_for_query(query);
        
        if agents.len() == 1 || !self.use_synthesis {
            // Single agent response
            let agent = agents.first().unwrap();
            let response = agent.process(query).await?;
            let content_clone = response.content.clone();
            
            let mut ctx = self.context.write().await;
            ctx.add_contribution(TeamAgentResponse {
                agent_id: response.agent_id.clone(),
                agent_name: response.agent_name.clone(),
                agent_role: response.agent_role.clone(),
                content: content_clone.clone(),
                reasoning: response.reasoning.clone(),
            });
            ctx.final_synthesis = Some(content_clone.clone());

            return Ok(TeamResponse {
                primary_response: content_clone,
                contributions: vec![response],
                synthesis: None,
                agents_used: vec![agent.name.clone()],
            });
        }

        // Multiple agents - gather contributions
        let mut handles = Vec::new();
        for agent in &agents {
            let agent = agent.clone();
            let query = query.to_string();
            handles.push(tokio::spawn(async move {
                agent.process(&query).await
            }));
        }

        // Wait for all responses
        let mut responses = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(response)) => responses.push(response),
                Ok(Err(e)) => tracing::warn!("Agent failed: {}", e),
                Err(e) => tracing::warn!("Task join failed: {}", e),
            }
        }

        // Store contributions in shared memory
        {
            let mut ctx = self.context.write().await;
            for response in &responses {
                ctx.add_contribution(TeamAgentResponse {
                    agent_id: response.agent_id.clone(),
                    agent_name: response.agent_name.clone(),
                    agent_role: response.agent_role.clone(),
                    content: response.content.clone(),
                    reasoning: response.reasoning.clone(),
                });
                
                // Store as team memory
                let memory = Memory::with_params(
                    format!("[{}] {}: {}", response.agent_role, response.agent_name, response.content),
                    "team".to_string(),
                    vec![
                        format!("team:{}", self.name),
                        format!("role:{}", response.agent_role),
                    ],
                    Some(MemoryType::Conversation),
                    true,
                );
                
                if let Err(e) = self.memory_store.store(&memory) {
                    tracing::warn!("Failed to store team memory: {}", e);
                }
            }
        }

        // Synthesize responses
        let synthesis = self.synthesize_responses(query, &responses).await?;

        Ok(TeamResponse {
            primary_response: synthesis.clone(),
            contributions: responses,
            synthesis: Some(synthesis),
            agents_used: agents.iter().map(|a| a.name.clone()).collect(),
        })
    }

    /// Synthesize multiple agent responses
    async fn synthesize_responses(&self, query: &str, responses: &[TeamAgentResponse]) -> Result<String, AgentError> {
        let ctx = self.context.read().await;
        let synthesis_prompt = ctx.synthesis_prompt(query);

        // Use one of the agents to synthesize (prefer Analyst or Assistant)
        let synthesizer = responses.iter()
            .find(|r| r.agent_role == AgentRole::Analyst || r.agent_role == AgentRole::Assistant)
            .cloned();

        if let Some(_synth) = synthesizer {
            // Create a synthesis message
            let messages = vec![
                Message::system("You are a synthesis specialist. Combine multiple perspectives into a coherent, comprehensive response.".to_string()),
                Message::user(synthesis_prompt),
            ];

            // Use the Agent to generate synthesis
            let agent_config = AgentConfig::default();
            let tool_registry = self.agents.values().next()
                .map(|a| a.tool_registry.clone())
                .unwrap_or_else(|| Arc::new(ToolRegistry::default()));

            let agent = Agent::new(
                self.config.clone(),
                agent_config,
                self.memory_store.clone(),
                self.embedding_client.clone(),
                tool_registry,
            ).map_err(|e| AgentError::SessionError(e.to_string()))?;

            let result = agent.chat(messages).await?;
            
            Ok(result.content)
        } else {
            // Fallback: join responses
            Ok(responses.iter()
                .map(|r| format!("[{}]: {}", r.agent_name, r.content))
                .join("\n\n"))
        }
    }

    /// Get agent by ID
    pub fn get_agent(&self, id: &str) -> Option<&TeamAgent> {
        self.agents.get(id).map(|a| a.as_ref())
    }

    /// Get all agents
    pub fn agents(&self) -> Vec<&TeamAgent> {
        self.agents.values().map(|a| a.as_ref()).collect()
    }

    /// Get agent count
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Get shared context
    pub async fn context(&self) -> SharedContext {
        self.context.read().await.clone()
    }

    /// Store a shared memory accessible to all team members
    pub async fn store_shared_memory(&self, content: &str, tags: Vec<String>) -> Result<(), AgentError> {
        let memory = Memory::with_params(
            content.to_string(),
            "team".to_string(),
            {
                let mut t = vec![format!("team:{}", self.name)];
                t.extend(tags);
                t
            },
            Some(MemoryType::Concept),
            true,
        );

        self.memory_store.store(&memory)
            .map_err(|e| AgentError::MemoryError(e.to_string()))?;

        Ok(())
    }

    /// Retrieve shared memories relevant to a query
    pub async fn get_shared_memories(&self, query: &str, limit: usize) -> Result<Vec<Memory>, AgentError> {
        let embedding = self.embedding_client.embed(query)
            .await
            .map_err(|e| AgentError::MemoryError(format!("Embedding failed: {}", e)))?;

        let results = self.memory_store.query(
            &embedding,
            "team",
            limit,
            0.5, // Lower threshold for shared memories
        ).map_err(|e| AgentError::MemoryError(e.to_string()))?;

        Ok(results.into_iter().map(|r| r.memory).collect())
    }
}

/// Response from team processing
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TeamResponse {
    /// The primary response to return
    pub primary_response: String,
    /// All contributions from team members
    pub contributions: Vec<TeamAgentResponse>,
    /// Synthesized response (if multiple agents were used)
    pub synthesis: Option<String>,
    /// Names of agents that contributed
    pub agents_used: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_setup() -> (AppConfig, Arc<SqliteMemoryStore>, Arc<EmbeddingClient>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        
        let config = AppConfig::default();
        let db_path = temp_dir.path().join("test.db");
        let index_path = temp_dir.path().join("index");
        
        let memory_store = Arc::new(
            SqliteMemoryStore::new(&db_path, &index_path).unwrap()
        );
        let embedding_client = Arc::new(EmbeddingClient::default());
        
        (config, memory_store, embedding_client, temp_dir)
    }

    #[test]
    fn test_agent_role_display() {
        assert_eq!(AgentRole::Assistant.to_string(), "Assistant");
        assert_eq!(AgentRole::Coder.to_string(), "Coder");
        assert_eq!(AgentRole::Researcher.to_string(), "Researcher");
        assert_eq!(AgentRole::Custom("Custom".to_string()).to_string(), "Custom");
    }

    #[test]
    fn test_agent_role_keywords() {
        assert!(AgentRole::Coder.keywords().contains(&"code"));
        assert!(AgentRole::Researcher.keywords().contains(&"research"));
        assert!(AgentRole::Writer.keywords().contains(&"write"));
        assert!(AgentRole::Analyst.keywords().contains(&"analyze"));
    }

    #[test]
    fn test_shared_context_synthesis_prompt() {
        let mut ctx = SharedContext::new();
        ctx.add_contribution(TeamAgentResponse {
            agent_id: "1".to_string(),
            agent_name: "Coder".to_string(),
            agent_role: AgentRole::Coder,
            content: "Here's the code solution.".to_string(),
            reasoning: None,
        });
        ctx.add_contribution(TeamAgentResponse {
            agent_id: "2".to_string(),
            agent_name: "Writer".to_string(),
            agent_role: AgentRole::Writer,
            content: "Here's the documentation.".to_string(),
            reasoning: None,
        });

        let prompt = ctx.synthesis_prompt("How do I solve problem X?");
        assert!(prompt.contains("How do I solve problem X?"));
        assert!(prompt.contains("Coder"));
        assert!(prompt.contains("Writer"));
    }

    #[tokio::test]
    async fn test_agent_team_creation() {
        let (config, memory_store, embedding_client, _temp_dir) = test_setup();
        
        let team = AgentTeam::new(
            "Test Team".to_string(),
            config,
            memory_store,
            embedding_client,
        ).unwrap();

        assert_eq!(team.name, "Test Team");
        assert_eq!(team.agent_count(), 0);
    }

    #[tokio::test]
    async fn test_find_agents_for_query() {
        let (config, memory_store, embedding_client, _temp_dir) = test_setup();
        
        let mut team = AgentTeam::new(
            "Test Team".to_string(),
            config.clone(),
            memory_store.clone(),
            embedding_client.clone(),
        ).unwrap();

        // Add test agents
        let agent = TeamAgent::new(
            "TestAgent".to_string(),
            AgentRole::Coder,
            AppConfig::default(),
            "You are a test agent.",
            memory_store.clone(),
            embedding_client.clone(),
            Arc::new(ToolRegistry::default()),
        ).unwrap();
        
        team.add_agent(agent);

        // Check that coder handles code queries
        let agents = team.find_agents_for_query("Write some code");
        assert!(!agents.is_empty());
    }

    #[tokio::test]
    async fn test_store_shared_memory() {
        let (config, memory_store, embedding_client, _temp_dir) = test_setup();
        
        let team = AgentTeam::new(
            "Test Team".to_string(),
            config,
            memory_store.clone(),
            embedding_client,
        ).unwrap();

        // Store a shared memory
        team.store_shared_memory(
            "Team knowledge: Use feature X for this task",
            vec!["knowledge".to_string()],
        ).await.unwrap();

        // Retrieve it
        let memories = memory_store.list("team", 10).unwrap();
        assert!(memories.iter().any(|m| m.content.contains("feature X")));
    }
}
