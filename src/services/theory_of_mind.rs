//! Theory of Mind Engine for AGI Agent
//! 
//! Phase 3 Feature: The agent can model user mental states including
//! beliefs, intentions, knowledge, and emotions to better understand
//! and predict user behavior.
//!
//! This module implements:
//! - User mental state modeling
//! - Belief tracking and inference
//! - Intention recognition
//! - Emotional state estimation
//! - Conversational context analysis

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Configuration for Theory of Mind engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TheoryOfMindConfig {
    /// Enable theory of mind modeling
    pub enabled: bool,
    /// Number of conversation turns to consider for context
    pub context_window: usize,
    /// Confidence threshold for belief inference
    pub belief_threshold: f32,
    /// Enable emotional state tracking
    pub track_emotions: bool,
    /// Enable intention recognition
    pub recognize_intentions: bool,
    /// Update interval in seconds
    pub update_interval_seconds: u64,
}

impl Default for TheoryOfMindConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            context_window: 10,
            belief_threshold: 0.6,
            track_emotions: true,
            recognize_intentions: true,
            update_interval_seconds: 60,
        }
    }
}

/// User mental state model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMentalState {
    pub user_id: String,
    pub beliefs: Vec<Belief>,
    pub intentions: Vec<Intention>,
    pub knowledge_state: KnowledgeState,
    pub emotional_state: EmotionalState,
    pub goals: Vec<Goal>,
    pub preferences: HashMap<String, Preference>,
    pub trust_level: f32,
    pub frustration_level: f32,
    pub engagement_level: f32,
    pub last_updated: DateTime<Utc>,
}

impl Default for UserMentalState {
    fn default() -> Self {
        Self {
            user_id: "default".to_string(),
            beliefs: Vec::new(),
            intentions: Vec::new(),
            knowledge_state: KnowledgeState::default(),
            emotional_state: EmotionalState::default(),
            goals: Vec::new(),
            preferences: HashMap::new(),
            trust_level: 0.5,
            frustration_level: 0.0,
            engagement_level: 0.5,
            last_updated: Utc::now(),
        }
    }
}

/// A belief held by the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Belief {
    pub id: String,
    pub content: String,
    pub confidence: f32,
    pub source: BeliefSource,
    pub created_at: DateTime<Utc>,
    pub last_reinforced: DateTime<Utc>,
    pub is_accurate: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BeliefSource {
    DirectStatement,
    Inference,
    Assumption,
    Correction,
}

/// An inferred user intention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intention {
    pub id: String,
    pub description: String,
    pub intention_type: IntentionType,
    pub confidence: f32,
    pub related_goals: Vec<String>,
    pub evidence: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentionType {
    /// User wants to learn something
    Learn,
    /// User wants to complete a task
    TaskCompletion,
    /// User wants information
    InformationSeeking,
    /// User wants help with a problem
    ProblemSolving,
    /// User wants to share information
    InformationSharing,
    /// User wants validation
    Validation,
    /// User wants to explore ideas
    Exploration,
    /// User wants emotional support
    EmotionalSupport,
    /// Unclear/Unknown
    Unknown,
}

impl IntentionType {
    pub fn from_query(query: &str) -> Self {
        let query_lower = query.to_lowercase();
        
        if query_lower.contains("how do i") || query_lower.contains("how to") || 
           query_lower.contains("what is") || query_lower.contains("explain") {
            IntentionType::Learn
        } else if query_lower.contains("help") || query_lower.contains("stuck") ||
                  query_lower.contains("doesn't work") || query_lower.contains("error") {
            IntentionType::ProblemSolving
        } else if query_lower.contains("find") || query_lower.contains("search") ||
                  query_lower.contains("look up") || query_lower.contains("what's") {
            IntentionType::InformationSeeking
        } else if query_lower.contains("can you") || query_lower.contains("please") ||
                  query_lower.contains("could you") || query_lower.contains("would you") {
            IntentionType::TaskCompletion
        } else if query_lower.contains("am i right") || query_lower.contains("correct") ||
                  query_lower.contains("is this right") {
            IntentionType::Validation
        } else if query_lower.contains("what if") || query_lower.contains("maybe") ||
                  query_lower.contains("thoughts on") {
            IntentionType::Exploration
        } else {
            IntentionType::Unknown
        }
    }
}

/// User's knowledge state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeState {
    pub known_concepts: Vec<KnownConcept>,
    pub unknown_concepts: Vec<String>,
    pub skill_levels: HashMap<String, SkillLevel>,
    pub misconceptions: Vec<Misconception>,
    pub expertise_areas: Vec<String>,
}

impl Default for KnowledgeState {
    fn default() -> Self {
        Self {
            known_concepts: Vec::new(),
            unknown_concepts: Vec::new(),
            skill_levels: HashMap::new(),
            misconceptions: Vec::new(),
            expertise_areas: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownConcept {
    pub concept: String,
    pub depth: f32,
    pub last_demonstrated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillLevel {
    Novice,
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

impl SkillLevel {
    #[allow(dead_code)]
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s < 0.2 => SkillLevel::Novice,
            s if s < 0.4 => SkillLevel::Beginner,
            s if s < 0.6 => SkillLevel::Intermediate,
            s if s < 0.8 => SkillLevel::Advanced,
            _ => SkillLevel::Expert,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Misconception {
    pub id: String,
    pub misconception: String,
    pub correction: String,
    pub evidence: Vec<String>,
    pub resolved: bool,
}

/// User's emotional state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalState {
    pub primary_emotion: Emotion,
    pub intensity: f32,
    pub emotion_history: Vec<EmotionSnapshot>,
    pub sentiment: f32,
    pub is_frustrated: bool,
    pub is_engaged: bool,
}

impl Default for EmotionalState {
    fn default() -> Self {
        Self {
            primary_emotion: Emotion::Neutral,
            intensity: 0.5,
            emotion_history: Vec::new(),
            sentiment: 0.5,
            is_frustrated: false,
            is_engaged: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Emotion {
    Neutral,
    Happy,
    Frustrated,
    Confused,
    Anxious,
    Excited,
    Sad,
    Angry,
    Curious,
    Satisfied,
    Dissatisfied,
    Surprised,
}

impl Emotion {
    pub fn from_sentiment(sentiment: f32, frustration: f32) -> Emotion {
        if frustration > 0.7 {
            Emotion::Frustrated
        } else if sentiment > 0.7 {
            Emotion::Happy
        } else if sentiment < 0.3 {
            Emotion::Sad
        } else if sentiment > 0.6 {
            Emotion::Curious
        } else {
            Emotion::Neutral
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionSnapshot {
    pub emotion: Emotion,
    pub intensity: f32,
    pub timestamp: DateTime<Utc>,
    pub context: String,
}

/// User goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub description: String,
    pub goal_type: GoalType,
    pub priority: u8,
    pub progress: f32,
    pub status: GoalStatus,
    pub related_intentions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalType {
    Learning,
    TaskCompletion,
    InformationGathering,
    ProblemSolving,
    Exploration,
    Social,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalStatus {
    Active,
    Completed,
    Abandoned,
    Blocked,
}

/// User preference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub preference_type: PreferenceType,
    pub value: String,
    pub strength: f32,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreferenceType {
    CommunicationStyle,
    DetailLevel,
    Tone,
    Pace,
    Format,
    Topic,
}

/// Theory of Mind analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToMAnalysis {
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub inferred_intentions: Vec<Intention>,
    pub belief_updates: Vec<Belief>,
    pub emotional_insight: String,
    pub recommended_response_style: ResponseStyle,
    pub confidence: f32,
}

/// Recommended response style based on user model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseStyle {
    pub detail_level: DetailLevel,
    pub tone: Tone,
    pub pace: Pace,
    pub include_reasoning: bool,
    pub use_examples: bool,
    pub ask_clarifying_questions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetailLevel {
    Brief,
    Moderate,
    Comprehensive,
    DeepDive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Tone {
    Formal,
    Neutral,
    Casual,
    Encouraging,
    Technical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Pace {
    Quick,
    Moderate,
    Thorough,
}

/// Conversation context for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ConversationContext {
    pub user_id: String,
    pub messages: Vec<MessageContext>,
    pub session_goals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContext {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub sentiment: Option<f32>,
}

/// Statistics about theory of mind modeling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToMStats {
    pub total_users: usize,
    pub active_users: usize,
    pub beliefs_tracked: usize,
    pub intentions_recognized: usize,
    pub emotions_analyzed: usize,
    pub average_confidence: f32,
}

/// The Theory of Mind Engine
pub struct TheoryOfMindEngine {
    config: TheoryOfMindConfig,
    user_models: Arc<RwLock<HashMap<String, UserMentalState>>>,
    conversation_history: Arc<RwLock<HashMap<String, Vec<MessageContext>>>>,
    #[allow(dead_code)]
    analysis_cache: Arc<RwLock<HashMap<String, ToMAnalysis>>>,
}

impl TheoryOfMindEngine {
    pub fn new(config: TheoryOfMindConfig) -> Self {
        Self {
            config,
            user_models: Arc::new(RwLock::new(HashMap::new())),
            conversation_history: Arc::new(RwLock::new(HashMap::new())),
            analysis_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update user mental state based on new messages
    pub async fn update_user_model(
        &self,
        user_id: &str,
        messages: &[MessageContext],
    ) -> Result<UserMentalState, String> {
        // Get or create user model
        let mut models = self.user_models.write().await;
        let model = models.entry(user_id.to_string()).or_insert_with(|| {
            let mut m = UserMentalState::default();
            m.user_id = user_id.to_string();
            m
        });

        // Update conversation history
        {
            let mut history = self.conversation_history.write().await;
            let user_history = history.entry(user_id.to_string()).or_insert_with(Vec::new);
            user_history.extend(messages.to_vec());
            
            // Trim to context window
            if user_history.len() > self.config.context_window {
                user_history.drain(0..user_history.len() - self.config.context_window);
            }
        }

        // Analyze messages and update beliefs
        for msg in messages {
            if msg.role == "user" {
                self.update_beliefs_from_message(model, msg);
                self.update_intentions_from_message(model, msg);
                self.update_emotional_state(model, msg);
                self.update_knowledge_from_message(model, msg);
                self.infer_preferences(model, msg);
            }
        }

        model.last_updated = Utc::now();
        Ok(model.clone())
    }

    /// Update beliefs based on user message
    fn update_beliefs_from_message(&self, model: &mut UserMentalState, msg: &MessageContext) {
        let content_lower = msg.content.to_lowercase();
        
        // Check for explicit beliefs (statements the user believes)
        let belief_indicators = ["i think", "i believe", "i feel", "in my opinion", "i assume"];
        for indicator in belief_indicators {
            if content_lower.contains(indicator) {
                // Extract the belief content
                let belief_content = msg.content.clone();
                let belief = Belief {
                    id: Uuid::new_v4().to_string(),
                    content: belief_content,
                    confidence: 0.8,
                    source: BeliefSource::DirectStatement,
                    created_at: msg.timestamp,
                    last_reinforced: msg.timestamp,
                    is_accurate: None,
                };
                model.beliefs.push(belief);
            }
        }

        // Check for corrections (user correcting the agent)
        if content_lower.contains("actually") || content_lower.contains("wait no") || 
           content_lower.contains("that's wrong") || content_lower.contains("i meant") {
            if let Some(last_belief) = model.beliefs.last_mut() {
                last_belief.is_accurate = Some(false);
            }
        }
    }

    /// Update intentions based on user message
    fn update_intentions_from_message(&self, model: &mut UserMentalState, msg: &MessageContext) {
        let intention_type = IntentionType::from_query(&msg.content);
        
        let intention = Intention {
            id: Uuid::new_v4().to_string(),
            description: format!("User wants to: {}", msg.content.chars().take(50).collect::<String>()),
            intention_type: intention_type.clone(),
            confidence: self.calculate_intention_confidence(&msg.content),
            related_goals: Vec::new(),
            evidence: vec![msg.content.clone()],
            created_at: msg.timestamp,
            satisfied: false,
        };

        // Add to intentions if not already present with similar type
        let already_exists = model.intentions.iter()
            .any(|i| i.intention_type == intention_type && !i.satisfied);
        
        if !already_exists {
            model.intentions.push(intention);
        }
    }

    /// Calculate confidence for intention inference
    fn calculate_intention_confidence(&self, message: &str) -> f32 {
        let message_lower = message.to_lowercase();
        let mut confidence: f32 = 0.5;

        // Increase confidence based on clear intent indicators
        let intent_words = ["help", "explain", "how", "what", "why", "can you", "please", "need", "want"];
        for word in intent_words {
            if message_lower.contains(word) {
                confidence += 0.1;
            }
        }

        // Questions indicate clear intent
        if message.contains('?') {
            confidence += 0.2;
        }

        // Commands suggest clear intent
        let command_indicators = ["do", "make", "create", "find", "show", "tell"];
        for cmd in command_indicators {
            if message_lower.starts_with(cmd) || message_lower.contains(&format!(" {} ", cmd)) {
                confidence += 0.15;
            }
        }

        confidence.min(1.0)
    }

    /// Update emotional state based on message
    fn update_emotional_state(&self, model: &mut UserMentalState, msg: &MessageContext) {
        let content_lower = msg.content.to_lowercase();
        
        // Detect frustration indicators
        let frustration_indicators = [
            "ugh", "seriously", "this is", "again", "still", "doesn't work",
            "tried", "frustrated", "annoying", "waste", "terrible"
        ];
        
        let frustration_count: usize = frustration_indicators.iter()
            .filter(|ind| content_lower.contains(*ind))
            .count();
        
        if frustration_count > 0 {
            model.frustration_level = (model.frustration_level + (frustration_count as f32 * 0.2)).min(1.0);
        } else {
            model.frustration_level = (model.frustration_level - 0.1).max(0.0);
        }

        // Detect engagement
        let engagement_indicators = ["thanks", "great", "perfect", "awesome", "cool", "interesting"];
        let engagement_count: usize = engagement_indicators.iter()
            .filter(|ind| content_lower.contains(*ind))
            .count();
        
        if engagement_count > 0 {
            model.engagement_level = (model.engagement_level + 0.1).min(1.0);
        }

        // Calculate sentiment (simplified)
        let positive_words = ["great", "good", "thanks", "awesome", "love", "perfect", "helpful"];
        let negative_words = ["bad", "wrong", "terrible", "hate", "awful", "horrible", "useless"];
        
        let pos_count: usize = positive_words.iter().filter(|w| content_lower.contains(*w)).count();
        let neg_count: usize = negative_words.iter().filter(|w| content_lower.contains(*w)).count();
        
        model.emotional_state.sentiment = if pos_count + neg_count > 0 {
            pos_count as f32 / (pos_count + neg_count) as f32
        } else {
            0.5
        };

        // Update emotion
        model.emotional_state.primary_emotion = Emotion::from_sentiment(
            model.emotional_state.sentiment,
            model.frustration_level
        );
        model.emotional_state.is_frustrated = model.frustration_level > 0.5;
        model.emotional_state.is_engaged = model.engagement_level > 0.3;

        // Record emotion snapshot
        model.emotional_state.emotion_history.push(EmotionSnapshot {
            emotion: model.emotional_state.primary_emotion.clone(),
            intensity: model.emotional_state.intensity,
            timestamp: msg.timestamp,
            context: msg.content.chars().take(100).collect(),
        });

        // Trim history
        if model.emotional_state.emotion_history.len() > 50 {
            model.emotional_state.emotion_history.drain(0..25);
        }
    }

    /// Update knowledge state from message
    fn update_knowledge_from_message(&self, model: &mut UserMentalState, msg: &MessageContext) {
        let content_lower = msg.content.to_lowercase();
        
        // Detect technical terms (simplified)
        let technical_indicators = [
            "api", "function", "code", "algorithm", "database", "server",
            "function", "variable", "class", "object", "method", "async",
            "rust", "python", "javascript", "golang", "docker", "kubernetes"
        ];

        for term in technical_indicators {
            if content_lower.contains(term) {
                // Check if user already knows this concept
                let exists = model.knowledge_state.known_concepts.iter()
                    .any(|k| k.concept.to_lowercase() == term);
                
                if !exists {
                    model.knowledge_state.known_concepts.push(KnownConcept {
                        concept: term.to_string(),
                        depth: 0.5,
                        last_demonstrated: msg.timestamp,
                    });
                }
            }
        }

        // Infer expertise from question sophistication
        let sophisticated_words = ["architecture", "optimization", "performance", "scalability", "concurrency"];
        let sophisticated_count: usize = sophisticated_words.iter()
            .filter(|w| content_lower.contains(*w))
            .count();
        
        if sophisticated_count > 1 {
            // User may be an expert
            let expertise = "advanced_technical".to_string();
            if !model.knowledge_state.expertise_areas.contains(&expertise) {
                model.knowledge_state.expertise_areas.push(expertise);
            }
        }
    }

    /// Infer user preferences from messages
    fn infer_preferences(&self, model: &mut UserMentalState, msg: &MessageContext) {
        let content_lower = msg.content.to_lowercase();
        
        // Communication style preference
        if content_lower.contains("be brief") || content_lower.contains("short answer") {
            model.preferences.insert("communication_style".to_string(), Preference {
                preference_type: PreferenceType::CommunicationStyle,
                value: "concise".to_string(),
                strength: 0.8,
                evidence_count: 1,
            });
        } else if content_lower.contains("explain in detail") || content_lower.contains("tell me more") {
            model.preferences.insert("communication_style".to_string(), Preference {
                preference_type: PreferenceType::CommunicationStyle,
                value: "detailed".to_string(),
                strength: 0.8,
                evidence_count: 1,
            });
        }

        // Format preference
        if content_lower.contains("show me code") || content_lower.contains("give me an example") {
            model.preferences.insert("format".to_string(), Preference {
                preference_type: PreferenceType::Format,
                value: "code_examples".to_string(),
                strength: 0.7,
                evidence_count: 1,
            });
        }
    }

    /// Mark an intention as satisfied
    pub async fn satisfy_intention(&self, user_id: &str, intention_id: &str) -> Result<(), String> {
        let mut models = self.user_models.write().await;
        
        if let Some(model) = models.get_mut(user_id) {
            if let Some(intention) = model.intentions.iter_mut().find(|i| i.id == intention_id) {
                intention.satisfied = true;
                return Ok(());
            }
            Err("Intention not found".to_string())
        } else {
            Err("User not found".to_string())
        }
    }

    /// Get user mental state
    pub async fn get_user_model(&self, user_id: &str) -> Option<UserMentalState> {
        let models = self.user_models.read().await;
        models.get(user_id).cloned()
    }

    /// Analyze user state and generate response recommendations
    pub async fn analyze_for_response(&self, user_id: &str) -> Option<ToMAnalysis> {
        let models = self.user_models.read().await;
        let model = models.get(user_id)?;
        
        // Determine response style based on user state
        let detail_level = if model.engagement_level > 0.7 {
            DetailLevel::Comprehensive
        } else if model.frustration_level > 0.5 {
            DetailLevel::Brief
        } else {
            DetailLevel::Moderate
        };

        let tone = if model.frustration_level > 0.6 {
            Tone::Encouraging
        } else if model.knowledge_state.expertise_areas.contains(&"advanced_technical".to_string()) {
            Tone::Technical
        } else {
            Tone::Neutral
        };

        let response_style = ResponseStyle {
            detail_level,
            tone,
            pace: Pace::Moderate,
            include_reasoning: model.engagement_level > 0.5,
            use_examples: true,
            ask_clarifying_questions: model.intentions.iter().any(|i| !i.satisfied && i.confidence < 0.7),
        };

        // Generate emotional insight
        let emotional_insight = if model.emotional_state.is_frustrated {
            "User appears frustrated. Consider being more empathetic and offering clear solutions.".to_string()
        } else if model.engagement_level > 0.7 {
            "User is highly engaged. Good opportunity for deeper explanations.".to_string()
        } else {
            "User is in neutral state. Standard helpful response appropriate.".to_string()
        };

        Some(ToMAnalysis {
            user_id: user_id.to_string(),
            timestamp: Utc::now(),
            inferred_intentions: model.intentions.iter().filter(|i| !i.satisfied).cloned().collect(),
            belief_updates: Vec::new(),
            emotional_insight,
            recommended_response_style: response_style,
            confidence: model.trust_level,
        })
    }

    /// Get statistics
    pub async fn get_stats(&self) -> ToMStats {
        let models = self.user_models.read().await;
        
        let total_beliefs: usize = models.values().map(|m| m.beliefs.len()).sum();
        let total_intentions: usize = models.values().map(|m| m.intentions.len()).sum();
        let total_emotions: usize = models.values()
            .map(|m| m.emotional_state.emotion_history.len())
            .sum();
        
        let avg_confidence = if !models.is_empty() {
            models.values().map(|m| m.trust_level).sum::<f32>() / models.len() as f32
        } else {
            0.0
        };

        ToMStats {
            total_users: models.len(),
            active_users: models.values().filter(|m| {
                (Utc::now() - m.last_updated).num_minutes() < 30
            }).count(),
            beliefs_tracked: total_beliefs,
            intentions_recognized: total_intentions,
            emotions_analyzed: total_emotions,
            average_confidence: avg_confidence,
        }
    }

    /// Clear user model
    pub async fn clear_user_model(&self, user_id: &str) {
        let mut models = self.user_models.write().await;
        models.remove(user_id);
        
        let mut history = self.conversation_history.write().await;
        history.remove(user_id);
    }

    /// Get all user models
    pub async fn get_all_user_models(&self) -> Vec<UserMentalState> {
        let models = self.user_models.read().await;
        models.values().cloned().collect()
    }

    /// Get conversation history for user
    pub async fn get_conversation_history(&self, user_id: &str) -> Vec<MessageContext> {
        let history = self.conversation_history.read().await;
        history.get(user_id).cloned().unwrap_or_default()
    }

    /// Update trust level based on interactions
    pub async fn update_trust(&self, user_id: &str, delta: f32) -> Result<(), String> {
        let mut models = self.user_models.write().await;
        
        if let Some(model) = models.get_mut(user_id) {
            model.trust_level = (model.trust_level + delta).clamp(0.0, 1.0);
            Ok(())
        } else {
            Err("User not found".to_string())
        }
    }
}

impl Default for TheoryOfMindEngine {
    fn default() -> Self {
        Self::new(TheoryOfMindConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_theory_of_mind_engine_creation() {
        let engine = TheoryOfMindEngine::new(TheoryOfMindConfig::default());
        assert!(engine.config.enabled);
    }

    #[tokio::test]
    async fn test_update_user_model() {
        let engine = TheoryOfMindEngine::new(TheoryOfMindConfig::default());
        
        let messages = vec![
            MessageContext {
                role: "user".to_string(),
                content: "How do I use async/await in Rust?".to_string(),
                timestamp: Utc::now(),
                sentiment: None,
            },
        ];
        
        let result = engine.update_user_model("user123", &messages).await;
        assert!(result.is_ok());
        
        let model = result.unwrap();
        assert_eq!(model.user_id, "user123");
        assert!(!model.intentions.is_empty());
    }

    #[tokio::test]
    async fn test_intention_recognition() {
        let intention_type = IntentionType::from_query("How do I build a REST API in Python?");
        assert!(matches!(intention_type, IntentionType::Learn | IntentionType::TaskCompletion));
    }

    #[tokio::test]
    async fn test_frustration_detection() {
        let engine = TheoryOfMindEngine::new(TheoryOfMindConfig::default());
        
        let messages = vec![
            MessageContext {
                role: "user".to_string(),
                content: "Ugh, this doesn't work again! I tried everything!".to_string(),
                timestamp: Utc::now(),
                sentiment: None,
            },
        ];
        
        let model = engine.update_user_model("frustrated_user", &messages).await.unwrap();
        assert!(model.emotional_state.is_frustrated);
        assert!(model.frustration_level > 0.5);
    }

    #[tokio::test]
    async fn test_skill_level_inference() {
        let skill = SkillLevel::from_score(0.85);
        assert!(matches!(skill, SkillLevel::Advanced | SkillLevel::Expert));
    }

    #[tokio::test]
    async fn test_get_stats() {
        let engine = TheoryOfMindEngine::new(TheoryOfMindConfig::default());
        let stats = engine.get_stats().await;
        
        assert_eq!(stats.total_users, 0);
        assert_eq!(stats.beliefs_tracked, 0);
    }

    #[tokio::test]
    async fn test_satisfy_intention() {
        let engine = TheoryOfMindEngine::new(TheoryOfMindConfig::default());
        
        // First add a user with messages
        let messages = vec![
            MessageContext {
                role: "user".to_string(),
                content: "What is Rust?".to_string(),
                timestamp: Utc::now(),
                sentiment: None,
            },
        ];
        
        engine.update_user_model("user1", &messages).await.unwrap();
        
        // Get the model to find intention ID
        let model = engine.get_user_model("user1").await.unwrap();
        let intention_id = model.intentions.first().map(|i| i.id.clone());
        
        if let Some(id) = intention_id {
            let result = engine.satisfy_intention("user1", &id).await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_response_style_recommendation() {
        let engine = TheoryOfMindEngine::new(TheoryOfMindConfig::default());
        
        let messages = vec![
            MessageContext {
                role: "user".to_string(),
                content: "Thanks! That's perfect!".to_string(),
                timestamp: Utc::now(),
                sentiment: None,
            },
        ];
        
        engine.update_user_model("happy_user", &messages).await.unwrap();
        
        let analysis = engine.analyze_for_response("happy_user").await;
        assert!(analysis.is_some());
        
        let analysis = analysis.unwrap();
        assert!(analysis.recommended_response_style.include_reasoning);
    }
}
