//! Reasoning engine
//! 
//! Implements the reasoning engine as specified in SPEC.md
//! Uses the LLM for actual chain-of-thought reasoning

use crate::agent::llm::LlmClient;
use crate::config::ModelConfig;
use crate::models::Message;
use std::collections::HashSet;

/// Reasoning engine for chain-of-thought processing
pub struct ReasoningEngine {
    depth: usize,
    enabled: bool,
    llm_client: Option<LlmClient>,
}

impl ReasoningEngine {
    /// Create a new reasoning engine with LLM client
    pub fn new(depth: usize) -> Self {
        Self {
            depth,
            enabled: true,
            llm_client: None,
        }
    }

    /// Create with LLM client for actual reasoning
    pub fn with_llm(depth: usize, model_config: ModelConfig) -> Self {
        Self {
            depth,
            enabled: true,
            llm_client: Some(LlmClient::new(model_config)),
        }
    }

    /// Enable or disable reasoning
    #[allow(dead_code)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set reasoning depth
    #[allow(dead_code)]
    pub fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
    }

    /// Perform reasoning on the context using the LLM
    pub async fn think(&self, context: &[Message]) -> Result<String, ReasoningError> {
        if !self.enabled {
            return Ok(String::new());
        }

        // If we have an LLM client, use it for actual reasoning
        if let Some(client) = &self.llm_client {
            return self.llm_think(client, context).await;
        }

        // Fallback to simple analysis
        self.simple_think(context).await
    }

    /// LLM-powered reasoning
    async fn llm_think(&self, client: &LlmClient, context: &[Message]) -> Result<String, ReasoningError> {
        // Build reasoning prompt
        let last_user_msg = context
            .iter()
            .rev()
            .find(|m| matches!(m.role, crate::models::Role::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let conversation_summary = self.summarize_conversation(context);

        let reasoning_prompt = format!(
            r#"You are a reasoning engine. Analyze the conversation and provide thoughtful reasoning.

Conversation Summary:
{}

Last User Query: {}

Perform {}-step reasoning:
1. What is the user asking for?
2. What information do I need to answer?
3. What is my plan for answering?

Think step by step and provide your reasoning in a clear format.
"#, 
            conversation_summary,
            last_user_msg,
            self.depth
        );

        let messages = vec![
            Message::system("You are a helpful reasoning assistant. Think deeply and provide structured reasoning.".to_string()),
            Message::user(reasoning_prompt),
        ];

        match client.chat(messages).await {
            Ok(reasoning) => Ok(reasoning),
            Err(e) => {
                tracing::warn!("LLM reasoning failed, falling back to simple: {}", e);
                self.simple_think(context).await
            }
        }
    }

    /// Summarize conversation for reasoning prompt
    fn summarize_conversation(&self, context: &[Message]) -> String {
        let mut summary = String::new();
        
        for msg in context.iter().take(10) {
            let role = match msg.role {
                crate::models::Role::System => "[System]",
                crate::models::Role::User => "[User]",
                crate::models::Role::Assistant => "[Assistant]",
            };
            
            let content = Self::truncate(&msg.content, 200);
            summary.push_str(&format!("{} {}\n", role, content));
        }
        
        if context.len() > 10 {
            summary.push_str(&format!("\n... and {} more messages", context.len() - 10));
        }
        
        summary
    }

    /// Simple fallback reasoning without LLM
    async fn simple_think(&self, context: &[Message]) -> Result<String, ReasoningError> {
        // Build reasoning chain
        let mut reasoning = String::new();
        reasoning.push_str("## Reasoning\n\n");

        // Analyze context
        let context_analysis = self.analyze_context(context);
        reasoning.push_str(&context_analysis);

        // Generate thought steps
        let thought_steps = self.generate_thought_steps(context);
        reasoning.push_str("\n\n### Thought Process\n\n");
        reasoning.push_str(&thought_steps);

        // Generate plan
        let plan = self.generate_plan(context);
        reasoning.push_str("\n\n### Plan\n\n");
        reasoning.push_str(&plan);

        Ok(reasoning)
    }

    /// Analyze the conversation context
    fn analyze_context(&self, context: &[Message]) -> String {
        let mut analysis = String::new();
        
        let user_messages: Vec<_> = context.iter()
            .filter(|m| matches!(m.role, crate::models::Role::User))
            .collect();
        
        let assistant_messages: Vec<_> = context.iter()
            .filter(|m| matches!(m.role, crate::models::Role::Assistant))
            .collect();

        analysis.push_str(&format!(
            "Context: {} user messages, {} assistant responses.\n",
            user_messages.len(),
            assistant_messages.len()
        ));

        // Extract key topics
        let topics = self.extract_topics(context);
        if !topics.is_empty() {
            analysis.push_str(&format!("Topics: {}\n", topics.join(", ")));
        }

        analysis
    }

    /// Extract key topics from context
    fn extract_topics(&self, context: &[Message]) -> Vec<String> {
        let mut topics = HashSet::new();
        
        for msg in context {
            // Simple keyword extraction
            let words: Vec<&str> = msg.content.split_whitespace().collect();
            
            // Look for capitalized words or specific patterns
            for word in words {
                if word.len() > 4 && word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    topics.insert(word.to_string());
                }
            }
        }
        
        topics.into_iter().take(5).collect()
    }

    /// Generate thought steps
    fn generate_thought_steps(&self, context: &[Message]) -> String {
        let mut steps = String::new();
        
        // Last user message
        if let Some(last_user) = context.iter().rev().find(|m| matches!(m.role, crate::models::Role::User)) {
            steps.push_str(&format!("1. User query: \"{}\"\n", 
                Self::truncate(&last_user.content, 100)));
        }

        steps.push_str("2. Understanding intent...\n");
        steps.push_str("3. Retrieving relevant context...\n");
        steps.push_str("4. Formulating response...\n");

        steps
    }

    /// Generate response plan
    fn generate_plan(&self, _context: &[Message]) -> String {
        let mut plan = String::new();
        
        plan.push_str("- Provide accurate and helpful information\n");
        plan.push_str("- Use clear, concise language\n");
        plan.push_str("- Include relevant examples if helpful\n");
        
        plan
    }

    /// Truncate text with ellipsis
    fn truncate(text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len.saturating_sub(3)])
        }
    }
}

impl Default for ReasoningEngine {
    fn default() -> Self {
        Self::new(3)
    }
}

/// Reasoning errors
#[derive(Debug, thiserror::Error)]
pub enum ReasoningError {
    #[error("Context too long: {0}")]
    #[allow(dead_code)]
    ContextTooLong(usize),
    
    #[error("Invalid context: {0}")]
    #[allow(dead_code)]
    InvalidContext(String),
    
    #[error("LLM error: {0}")]
    #[allow(dead_code)]
    LlmError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reasoning_disabled() {
        let mut engine = ReasoningEngine::new(3);
        engine.set_enabled(false);
        
        let result = engine.think(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_reasoning_enabled() {
        let engine = ReasoningEngine::new(3);
        
        let messages = vec![
            Message::user("Tell me about Rust".to_string()),
            Message::assistant("Rust is a programming language...".to_string()),
            Message::user("What about memory safety?".to_string()),
        ];
        
        let result = engine.think(&messages).await.unwrap();
        
        assert!(result.contains("## Reasoning"));
        assert!(result.contains("## Reasoning\n\nContext:"));
        assert!(result.contains("Thought Process"));
        assert!(result.contains("Plan"));
    }

    #[test]
    fn test_extract_topics() {
        let engine = ReasoningEngine::new(3);
        
        let messages = vec![
            Message::user("Tell me about Rust and WebAssembly".to_string()),
        ];
        
        let topics = engine.extract_topics(&messages);
        
        // Should find at least some topics
        assert!(!topics.is_empty());
    }

    #[test]
    fn test_truncate() {
        assert_eq!(ReasoningEngine::truncate("short", 100), "short");
        assert_eq!(ReasoningEngine::truncate("this is a long message", 10), "this is...");
        assert_eq!(ReasoningEngine::truncate("exactly10!", 10), "exactly10!");
    }

    #[test]
    fn test_analyze_context() {
        let engine = ReasoningEngine::new(3);
        
        let messages = vec![
            Message::system("You are helpful".to_string()),
            Message::user("Hello".to_string()),
            Message::assistant("Hi!".to_string()),
        ];
        
        let analysis = engine.analyze_context(&messages);
        
        assert!(analysis.contains("1 user messages"));
        assert!(analysis.contains("1 assistant responses"));
    }

    #[test]
    fn test_engine_configuration() {
        let mut engine = ReasoningEngine::new(5);
        
        assert_eq!(engine.depth, 5);
        
        engine.set_depth(10);
        assert_eq!(engine.depth, 10);
        
        engine.set_enabled(false);
        assert!(!engine.enabled);
        
        engine.set_enabled(true);
        assert!(engine.enabled);
    }
}
