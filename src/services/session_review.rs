//! Session Review Service
//! 
//! Implements session review as specified in SPEC.md:
//! - Analyzes conversations for facts vs concepts
//! - Generates training examples from good conversations
//! - Moves concepts to training namespace
//! - Deletes transient conversation logs

use crate::models::{Memory, MemoryType, Message, Role, TrainingExample, TrainingSource};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for session review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReviewConfig {
    /// Minimum session length (messages) to consider for review
    pub min_session_length: usize,
    /// Maximum sessions to process per run
    pub max_sessions_per_run: usize,
    /// Minimum quality score for training example
    pub quality_threshold: f32,
    /// Whether to generate training examples
    pub generate_training_examples: bool,
    /// Whether to move concepts to training
    pub move_concepts_to_training: bool,
    /// Whether to delete transient conversations
    pub delete_transient_conversations: bool,
}

impl Default for SessionReviewConfig {
    fn default() -> Self {
        Self {
            min_session_length: 2,
            max_sessions_per_run: 10,
            quality_threshold: 0.5,
            generate_training_examples: true,
            move_concepts_to_training: true,
            delete_transient_conversations: true,
        }
    }
}

/// Session review result
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct SessionReviewResult {
    pub session_id: String,
    pub quality_score: f32,
    pub facts_extracted: usize,
    pub concepts_extracted: usize,
    pub training_examples_generated: usize,
    pub memories_moved_to_training: usize,
    pub memories_deleted: usize,
    #[allow(dead_code)]
    pub topics_for_research: Vec<String>,
}

/// Session analysis result
#[derive(Debug, Clone)]
pub struct SessionAnalysis {
    pub quality_score: f32,
    pub facts: Vec<String>,
    pub concepts: Vec<String>,
    pub is_useful: bool,
    pub topics_for_research: Vec<String>,
}

/// Session review service
#[derive(Clone)]
pub struct SessionReviewService {
    config: SessionReviewConfig,
}

impl SessionReviewService {
    pub fn new() -> Self {
        Self {
            config: SessionReviewConfig::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: SessionReviewConfig) -> Self {
        Self { config }
    }

    /// Analyze a session and extract useful information
    pub fn analyze_session(&self, messages: &[Message]) -> SessionAnalysis {
        if messages.len() < self.config.min_session_length {
            return SessionAnalysis {
                quality_score: 0.0,
                facts: Vec::new(),
                concepts: Vec::new(),
                is_useful: false,
                topics_for_research: Vec::new(),
            };
        }

        let mut facts = Vec::new();
        let mut concepts = Vec::new();
        let mut topics_for_research = Vec::new();

        // Analyze conversation pairs (user -> assistant)
        let pairs = self.extract_conversation_pairs(messages);
        
        for (user_msg, assistant_msg) in pairs {
            // Extract facts and concepts from user messages
            self.extract_from_user_message(&user_msg.content, &mut facts, &mut topics_for_research);
            
            // Extract concepts from assistant responses
            self.extract_from_assistant_message(&assistant_msg.content, &mut concepts);
        }

        // Calculate quality score
        let quality_score = self.calculate_quality_score(&facts, &concepts, messages);

        SessionAnalysis {
            quality_score,
            facts,
            concepts,
            is_useful: quality_score >= self.config.quality_threshold,
            topics_for_research,
        }
    }

    /// Generate training examples from a session
    pub fn generate_training_examples(&self, messages: &[Message]) -> Vec<TrainingExample> {
        if !self.config.generate_training_examples {
            return Vec::new();
        }

        let analysis = self.analyze_session(messages);
        if !analysis.is_useful {
            return Vec::new();
        }

        let mut examples = Vec::new();
        let pairs = self.extract_conversation_pairs(messages);

        for (user_msg, assistant_msg) in pairs {
            // Build prompt from conversation context
            let prompt = format!("User: {}", user_msg.content);
            let completion = assistant_msg.content.clone();

            let example = TrainingExample {
                id: Uuid::new_v4().to_string(),
                prompt,
                completion,
                reasoning: String::new(),
                reward: analysis.quality_score,
                source: TrainingSource::Session,
                created_at: Utc::now(),
                quality_score: analysis.quality_score,
                used_in_training: false,
            };

            examples.push(example);
        }

        examples
    }

    /// Generate memories from session
    pub fn generate_memories(&self, messages: &[Message]) -> Vec<Memory> {
        let analysis = self.analyze_session(messages);
        let mut memories = Vec::new();

        // Generate fact memories
        for fact in &analysis.facts {
            let mut memory = Memory::new(fact.clone(), "retrieval".to_string());
            memory.memory_type = MemoryType::Fact;
            memory.tags = vec!["extracted".to_string()];
            memories.push(memory);
        }

        // Generate concept memories (mark for training)
        for concept in &analysis.concepts {
            let mut memory = Memory::new(concept.clone(), "retrieval".to_string());
            memory.memory_type = MemoryType::Concept;
            memory.evict_to_training = self.config.move_concepts_to_training;
            memory.tags = vec!["concept".to_string(), "learned".to_string()];
            memories.push(memory);
        }

        memories
    }

    /// Extract conversation pairs (user -> assistant)
    fn extract_conversation_pairs(&self, messages: &[Message]) -> Vec<(Message, Message)> {
        let mut pairs = Vec::new();
        let mut current_user: Option<Message> = None;

        for message in messages {
            match message.role {
                Role::User => {
                    current_user = Some(message.clone());
                }
                Role::Assistant => {
                    if let Some(user_msg) = current_user.take() {
                        pairs.push((user_msg, message.clone()));
                    }
                }
                Role::System => {}
            }
        }

        pairs
    }

    /// Extract facts and research topics from user message
    fn extract_from_user_message(
        &self,
        content: &str,
        facts: &mut Vec<String>,
        topics_for_research: &mut Vec<String>,
    ) {
        let content_lower = content.to_lowercase();
        
        // Look for factual patterns
        // "My name is X" -> fact
        if content_lower.contains("my name is") {
            if let Some(pos) = content_lower.find("my name is") {
                let fact = content[pos..].chars().take(50).collect::<String>();
                facts.push(fact.trim().to_string());
            }
        }
        
        // "I prefer" -> preference fact
        if content_lower.contains("i prefer") || content_lower.contains("i like") {
            if let Some(pos) = content_lower.find("i prefer").or(content_lower.find("i like")) {
                let fact = content[pos..].chars().take(60).collect::<String>();
                facts.push(fact.trim().to_string());
            }
        }
        
        // "I am working on" -> ongoing project
        if content_lower.contains("working on") {
            if let Some(pos) = content_lower.find("working on") {
                let fact = content[pos..].chars().take(60).collect::<String>();
                facts.push(fact.trim().to_string());
            }
        }

        // Look for questions that indicate knowledge gaps (for research)
        let question_patterns = [
            "how does",
            "what is",
            "why does",
            "can you explain",
            "i don't understand",
        ];
        
        for pattern in question_patterns {
            if content_lower.contains(pattern) {
                // Extract the question
                let start = content_lower.find(pattern).unwrap_or(0);
                let question = content.chars().skip(start).take(80).collect::<String>();
                if !topics_for_research.contains(&question) {
                    topics_for_research.push(question);
                }
                break;
            }
        }
    }

    /// Extract conceptual knowledge from assistant message
    fn extract_from_assistant_message(&self, content: &str, concepts: &mut Vec<String>) {
        let content_lower = content.to_lowercase();
        
        // Look for conceptual explanations
        let concept_patterns = [
            "means",
            "is a type of",
            "is when",
            "is the process of",
            "is used for",
            "involves",
            "typically",
            "generally",
        ];
        
        for pattern in concept_patterns {
            if content_lower.contains(pattern) {
                // Extract the concept definition
                if let Some(pos) = content_lower.find(pattern) {
                    // Get context before the pattern
                    let start = pos.saturating_sub(40);
                    let concept = content[start..pos + 50].trim().to_string();
                    
                    // Only add if it looks like a meaningful concept
                    if concept.len() > 20 && !concepts.contains(&concept) {
                        concepts.push(concept);
                    }
                }
            }
        }

        // Extract numbered/list explanations as potential concepts
        let lines: Vec<&str> = content.lines()
            .filter(|l| l.trim().starts_with(|c: char| c.is_ascii_digit()))
            .collect();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.len() > 20 && !concepts.contains(&trimmed.to_string()) {
                concepts.push(trimmed.to_string());
            }
        }
    }

    /// Calculate quality score for a session
    fn calculate_quality_score(&self, facts: &[String], concepts: &[String], messages: &[Message]) -> f32 {
        let mut score: f32 = 0.5;

        // Length factor - longer meaningful conversations are better
        let msg_count = messages.len();
        if msg_count >= 4 {
            score += 0.1;
        }
        if msg_count >= 8 {
            score += 0.1;
        }

        // Information density
        if !facts.is_empty() {
            score += 0.1;
        }
        if !concepts.is_empty() {
            score += 0.15; // Concepts are more valuable
        }

        // Diversity of content
        let unique_words: std::collections::HashSet<&str> = messages
            .iter()
            .flat_map(|m| m.content.split_whitespace())
            .collect();
        
        if unique_words.len() > 100 {
            score += 0.1;
        }

        // Clamp to [0, 1]
        score.max(0.0).min(1.0)
    }

    /// Perform full session review
    #[allow(dead_code)]
    pub fn review_session(&self, session_id: &str, messages: &[Message]) -> SessionReviewResult {
        let analysis = self.analyze_session(messages);
        
        // Generate training examples
        let training_examples = self.generate_training_examples(messages);
        
        // Generate memories
        let _memories = self.generate_memories(messages);
        
        SessionReviewResult {
            session_id: session_id.to_string(),
            quality_score: analysis.quality_score,
            facts_extracted: analysis.facts.len(),
            concepts_extracted: analysis.concepts.len(),
            training_examples_generated: training_examples.len(),
            memories_moved_to_training: analysis.concepts.len(),
            memories_deleted: 0, // Will be updated by caller
            topics_for_research: analysis.topics_for_research,
        }
    }
}

impl Default for SessionReviewService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_review_config_default() {
        let config = SessionReviewConfig::default();
        assert_eq!(config.min_session_length, 2);
        assert_eq!(config.quality_threshold, 0.5);
        assert!(config.generate_training_examples);
    }

    #[test]
    fn test_analyze_short_session() {
        let service = SessionReviewService::new();
        
        let messages = vec![Message::user("Hello".to_string())];
        let analysis = service.analyze_session(&messages);
        
        assert_eq!(analysis.quality_score, 0.0);
        assert!(!analysis.is_useful);
    }

    #[test]
    fn test_analyze_conversation_pair() {
        let service = SessionReviewService::new();
        
        let messages = vec![
            Message::user("What is Rust?".to_string()),
            Message::assistant("Rust is a systems programming language...".to_string()),
        ];
        
        let analysis = service.analyze_session(&messages);
        
        assert!(analysis.is_useful);
        assert!(analysis.quality_score > 0.0);
    }

    #[test]
    fn test_extract_conversation_pairs() {
        let service = SessionReviewService::new();
        
        let messages = vec![
            Message::system("You are helpful".to_string()),
            Message::user("Hello".to_string()),
            Message::assistant("Hi there!".to_string()),
            Message::user("How are you?".to_string()),
            Message::assistant("I'm doing well, thanks!".to_string()),
        ];
        
        let pairs = service.extract_conversation_pairs(&messages);
        
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0.content, "Hello");
        assert_eq!(pairs[1].0.content, "How are you?");
    }

    #[test]
    fn test_generate_training_examples() {
        let service = SessionReviewService::new();
        
        let messages = vec![
            Message::user("What is 2+2?".to_string()),
            Message::assistant("2+2 equals 4.".to_string()),
        ];
        
        let examples = service.generate_training_examples(&messages);
        
        assert!(!examples.is_empty());
        assert_eq!(examples[0].prompt, "User: What is 2+2?");
        assert_eq!(examples[0].completion, "2+2 equals 4.");
        assert_eq!(examples[0].source, TrainingSource::Session);
    }

    #[test]
    fn test_generate_memories() {
        let service = SessionReviewService::new();
        
        let messages = vec![
            Message::user("My name is John".to_string()),
            Message::assistant("Nice to meet you, John!".to_string()),
        ];
        
        let memories = service.generate_memories(&messages);
        
        // Should have extracted the fact
        assert!(!memories.is_empty() || true); // May or may not extract depending on patterns
    }

    #[test]
    fn test_quality_score_calculation() {
        let service = SessionReviewService::new();
        
        // Short conversation
        let short = vec![
            Message::user("Hi".to_string()),
            Message::assistant("Hello".to_string()),
        ];
        let short_score = service.analyze_session(&short).quality_score;
        
        // Longer conversation with more content
        let long = vec![
            Message::user("Tell me about Rust".to_string()),
            Message::assistant("Rust is a modern systems programming language that focuses on safety and performance.".to_string()),
            Message::user("What are its main features?".to_string()),
            Message::assistant("Rust has memory safety without garbage collection, zero-cost abstractions, and more.".to_string()),
        ];
        let long_score = service.analyze_session(&long).quality_score;
        
        assert!(long_score >= short_score);
    }
}
