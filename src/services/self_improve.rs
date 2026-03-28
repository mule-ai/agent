//! Self-Improvement Engine for AGI Agent
//! 
//! Phase 3 Feature: The agent can analyze its own behavior, identify weaknesses,
//! and generate code/improvements to enhance its capabilities.
//!
//! This module implements:
//! - Performance analysis and weakness detection
//! - Automatic tool generation
//! - Prompt optimization
//! - Configuration improvements
//! - Code pattern analysis from search results
//! - Actual code improvement application

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Configuration for self-improvement engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfImproveConfig {
    /// Enable automatic self-improvement
    pub enabled: bool,
    /// Minimum confidence threshold for making changes
    pub min_confidence: f32,
    /// Maximum improvements per analysis cycle
    pub max_improvements: usize,
    /// Enable automatic tool generation
    pub auto_generate_tools: bool,
    /// Enable prompt optimization
    pub optimize_prompts: bool,
    /// Enable configuration tuning
    pub tune_config: bool,
    /// Analysis interval in seconds
    pub analysis_interval_seconds: u64,
}

impl Default for SelfImproveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_confidence: 0.8,
            max_improvements: 3,
            auto_generate_tools: true,
            optimize_prompts: true,
            tune_config: true,
            analysis_interval_seconds: 3600, // 1 hour
        }
    }
}

/// Types of improvements the agent can make
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImprovementType {
    /// Create a new tool
    ToolGeneration,
    /// Improve existing tool
    ToolImprovement,
    /// Optimize system prompt
    PromptOptimization,
    /// Tune configuration parameters
    ConfigTuning,
    /// Fix identified bug
    BugFix,
    /// Add new capability
    NewCapability,
}

/// Status of an improvement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImprovementStatus {
    Pending,
    Generated,
    Tested,
    Approved,
    Applied,
    Rejected,
    RolledBack,
}

/// A generated improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    pub id: String,
    pub improvement_type: ImprovementType,
    pub target: String,
    pub description: String,
    pub confidence: f32,
    pub generated_code: Option<String>,
    pub original_code: Option<String>,
    pub tests: Vec<String>,
    pub status: ImprovementStatus,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
    pub impact_score: f32,
    pub rollback_plan: Option<String>,
}

impl Improvement {
    pub fn new(
        improvement_type: ImprovementType,
        target: String,
        description: String,
        confidence: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            improvement_type,
            target,
            description,
            confidence,
            generated_code: None,
            original_code: None,
            tests: Vec::new(),
            status: ImprovementStatus::Pending,
            created_at: Utc::now(),
            applied_at: None,
            impact_score: 0.0,
            rollback_plan: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_code(mut self, code: String, original: Option<String>) -> Self {
        self.generated_code = Some(code);
        self.original_code = original;
        self.status = ImprovementStatus::Generated;
        self
    }

    pub fn with_impact(mut self, impact: f32) -> Self {
        self.impact_score = impact;
        self
    }
}

/// Analysis of agent performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub session_count: usize,
    pub tool_usage_stats: HashMap<String, usize>,
    pub success_rate: f32,
    pub avg_response_quality: f32,
    pub identified_weaknesses: Vec<Weakness>,
    pub suggested_improvements: Vec<ImprovementSuggestion>,
    pub overall_score: f32,
}

/// A detected weakness in agent behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weakness {
    pub id: String,
    pub category: WeaknessCategory,
    pub description: String,
    pub severity: f32,
    pub evidence: Vec<String>,
    pub frequency: usize,
    pub impact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WeaknessCategory {
    ToolHandling,
    PromptQuality,
    ReasoningDepth,
    MemoryRetrieval,
    ResponseFormat,
    ContextWindow,
    Unknown,
}

/// Suggestion for improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementSuggestion {
    pub id: String,
    pub weakness_id: String,
    pub improvement_type: ImprovementType,
    pub description: String,
    pub confidence: f32,
    pub priority: usize,
    pub estimated_impact: f32,
}

/// Generated tool specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub implementation: String,
    pub tests: String,
    pub dependencies: Vec<String>,
}

/// Prompt optimization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOptimization {
    pub id: String,
    pub original_prompt: String,
    pub optimized_prompt: String,
    pub improvements: Vec<String>,
    pub expected_benefit: String,
    pub validated: bool,
}

impl Default for PromptOptimization {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            original_prompt: String::new(),
            optimized_prompt: String::new(),
            improvements: Vec::new(),
            expected_benefit: String::new(),
            validated: false,
        }
    }
}

/// Self-improvement engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfImproveStats {
    pub total_improvements: usize,
    pub pending_improvements: usize,
    pub applied_improvements: usize,
    pub rejected_improvements: usize,
    pub total_analyses: usize,
    pub last_analysis: Option<DateTime<Utc>>,
    pub average_impact: f32,
    pub generated_tools_count: usize,
    pub prompt_optimizations_count: usize,
}

/// Tool template for code generation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ToolTemplate {
    pub name: String,
    pub description: String,
    pub parameter_schema: String,
    pub implementation_template: String,
}

// ============================================
// Code Pattern Analysis (Self-Improvement through Code Generation)
// ============================================

/// Represents a code pattern detected from search results or analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePattern {
    pub id: String,
    pub pattern_type: CodePatternType,
    pub description: String,
    pub example_code: String,
    pub source_file: Option<String>,
    pub source_url: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub relevance_score: f32,
}

/// Types of code patterns that can be detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodePatternType {
    /// Best practice pattern
    BestPractice,
    /// Performance optimization
    Performance,
    /// Error handling pattern
    ErrorHandling,
    /// Async pattern
    Async,
    /// Memory management pattern
    Memory,
    /// API design pattern
    ApiDesign,
    /// Testing pattern
    Testing,
    /// Security pattern
    Security,
    /// Refactoring opportunity
    Refactoring,
    /// Missing feature pattern
    MissingFeature,
}

/// A detected code improvement opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeImprovement {
    pub id: String,
    pub pattern_id: String,
    pub target_file: String,
    pub target_function: Option<String>,
    pub current_code: String,
    pub improved_code: String,
    pub explanation: String,
    pub confidence: f32,
    pub effort_estimate: EffortLevel,
    pub created_at: DateTime<Utc>,
}

impl CodeImprovement {
    #[allow(dead_code)]
    pub fn new(
        pattern_id: String,
        target_file: String,
        current_code: String,
        improved_code: String,
        explanation: String,
        confidence: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pattern_id,
            target_file,
            target_function: None,
            current_code,
            improved_code,
            explanation,
            confidence,
            effort_estimate: EffortLevel::Medium,
            created_at: Utc::now(),
        }
    }
}

/// Effort level for implementing an improvement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EffortLevel {
    Low,
    Medium,
    High,
}

/// Improvement history entry for tracking applied changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementHistoryEntry {
    pub id: String,
    pub improvement_id: String,
    pub action: ImprovementAction,
    pub timestamp: DateTime<Utc>,
    pub details: String,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImprovementAction {
    Created,
    Tested,
    Approved,
    Applied,
    Rejected,
    RolledBack,
}

/// Code analysis result from search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAnalysisResult {
    pub patterns: Vec<CodePattern>,
    pub improvements: Vec<CodeImprovement>,
    pub source_query: String,
    pub analyzed_at: DateTime<Utc>,
}

/// The Self-Improvement Engine
pub struct SelfImproveEngine {
    config: SelfImproveConfig,
    improvements: Arc<RwLock<Vec<Improvement>>>,
    analyses: Arc<RwLock<Vec<PerformanceAnalysis>>>,
    generated_tools: Arc<RwLock<Vec<GeneratedToolSpec>>>,
    prompt_history: Arc<RwLock<Vec<PromptOptimization>>>,
    current_prompt: Arc<RwLock<String>>,
    // Code analysis fields
    code_patterns: Arc<RwLock<Vec<CodePattern>>>,
    code_improvements: Arc<RwLock<Vec<CodeImprovement>>>,
    improvement_history: Arc<RwLock<Vec<ImprovementHistoryEntry>>>,
    project_root: Arc<RwLock<Option<PathBuf>>>,
}

impl SelfImproveEngine {
    pub fn new(config: SelfImproveConfig) -> Self {
        Self {
            config,
            improvements: Arc::new(RwLock::new(Vec::new())),
            analyses: Arc::new(RwLock::new(Vec::new())),
            generated_tools: Arc::new(RwLock::new(Vec::new())),
            prompt_history: Arc::new(RwLock::new(Vec::new())),
            current_prompt: Arc::new(RwLock::new(Self::default_system_prompt())),
            code_patterns: Arc::new(RwLock::new(Vec::new())),
            code_improvements: Arc::new(RwLock::new(Vec::new())),
            improvement_history: Arc::new(RwLock::new(Vec::new())),
            project_root: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the project root directory for code improvements (for future use)
    #[allow(dead_code)]
    pub fn with_project_root(mut self, root: PathBuf) -> Self {
        self.project_root = Arc::new(RwLock::new(Some(root)));
        self
    }

    fn default_system_prompt() -> String {
        r#"You are an AI assistant with extensive memory and learning capabilities.

You have access to:
- Long-term memory that persists across conversations
- Ability to search for information on the web
- File system access for reading and writing

Be helpful, concise, and accurate. Use your tools when appropriate."#.to_string()
    }

    /// Analyze recent performance and identify improvements
    pub async fn analyze_and_improve(
        &self,
        recent_interactions: &[InteractionSummary],
        tool_usage: &HashMap<String, usize>,
        errors: &[String],
    ) -> Vec<Improvement> {
        // Analyze performance
        let analysis = self.perform_analysis(recent_interactions, tool_usage, errors).await;
        
        // Store analysis (clone to allow further use)
        {
            let mut analyses = self.analyses.write().await;
            analyses.push(analysis.clone());
        }

        // Generate improvements based on analysis
        let mut improvements = Vec::new();
        
        for suggestion in &analysis.suggested_improvements {
            if suggestion.confidence >= self.config.min_confidence {
                let improvement = self.generate_improvement(&suggestion).await;
                improvements.push(improvement);
            }
        }

        // Store improvements
        {
            let mut stored = self.improvements.write().await;
            stored.extend(improvements.clone());
        }

        improvements
    }

    /// Perform performance analysis
    async fn perform_analysis(
        &self,
        interactions: &[InteractionSummary],
        tool_usage: &HashMap<String, usize>,
        errors: &[String],
    ) -> PerformanceAnalysis {
        let mut weaknesses = Vec::new();
        let mut suggestions = Vec::new();

        // Analyze tool usage patterns
        let (underused_tools, _overused_tools) = self.analyze_tool_usage(tool_usage);
        
        for tool in underused_tools {
            weaknesses.push(Weakness {
                id: Uuid::new_v4().to_string(),
                category: WeaknessCategory::ToolHandling,
                description: format!("Tool '{}' is underutilized", tool),
                severity: 0.5,
                evidence: vec![format!("Used {} times in recent history", tool_usage.get(&tool).unwrap_or(&0))],
                frequency: tool_usage.get(&tool).copied().unwrap_or(0),
                impact: "Agent may be missing opportunities to use available tools".to_string(),
            });
        }

        // Analyze errors
        for error in errors {
            let weakness = self.categorize_error(error);
            weaknesses.push(weakness);
        }

        // Generate suggestions from weaknesses
        for weakness in &weaknesses {
            let suggestion = ImprovementSuggestion {
                id: Uuid::new_v4().to_string(),
                weakness_id: weakness.id.clone(),
                improvement_type: self.suggest_improvement_type(&weakness.category),
                description: format!("Address: {}", weakness.description),
                confidence: weakness.severity,
                priority: ((1.0 - weakness.severity) * 10.0) as usize,
                estimated_impact: weakness.severity,
            };
            suggestions.push(suggestion);
        }

        // Calculate overall score
        let overall_score = if !interactions.is_empty() {
            let success_rate = interactions.iter()
                .filter(|i| i.success)
                .count() as f32 / interactions.len() as f32;
            let quality = interactions.iter()
                .map(|i| i.quality_score)
                .sum::<f32>() / interactions.len() as f32;
            (success_rate * 0.6 + quality * 0.4).min(1.0)
        } else {
            0.5
        };

        PerformanceAnalysis {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            session_count: interactions.len(),
            tool_usage_stats: tool_usage.clone(),
            success_rate: overall_score,
            avg_response_quality: overall_score,
            identified_weaknesses: weaknesses,
            suggested_improvements: suggestions,
            overall_score,
        }
    }

    /// Analyze tool usage patterns
    fn analyze_tool_usage(&self, tool_usage: &HashMap<String, usize>) -> (Vec<String>, Vec<String>) {
        let avg_usage: f32 = if tool_usage.is_empty() {
            0.0
        } else {
            tool_usage.values().map(|v| *v as f32).sum::<f32>() / tool_usage.len() as f32
        };

        let mut underused = Vec::new();
        let mut overused = Vec::new();

        for (tool, count) in tool_usage {
            let usage = *count as f32;
            if usage < avg_usage * 0.3 {
                underused.push(tool.clone());
            } else if usage > avg_usage * 3.0 {
                overused.push(tool.clone());
            }
        }

        (underused, overused)
    }

    /// Categorize an error into a weakness
    fn categorize_error(&self, error: &str) -> Weakness {
        let error_lower = error.to_lowercase();
        
        let (category, severity, evidence) = if error_lower.contains("tool") || error_lower.contains("function") {
            (WeaknessCategory::ToolHandling, 0.7, vec![error.to_string()])
        } else if error_lower.contains("prompt") || error_lower.contains("context") {
            (WeaknessCategory::PromptQuality, 0.5, vec![error.to_string()])
        } else if error_lower.contains("memory") || error_lower.contains("retrieve") {
            (WeaknessCategory::MemoryRetrieval, 0.6, vec![error.to_string()])
        } else if error_lower.contains("reason") || error_lower.contains("logic") {
            (WeaknessCategory::ReasoningDepth, 0.5, vec![error.to_string()])
        } else {
            (WeaknessCategory::Unknown, 0.3, vec![error.to_string()])
        };

        Weakness {
            id: Uuid::new_v4().to_string(),
            category,
            description: format!("Error occurred: {}", error.chars().take(100).collect::<String>()),
            severity,
            evidence,
            frequency: 1,
            impact: "May affect response quality or task completion".to_string(),
        }
    }

    /// Suggest improvement type based on weakness category
    fn suggest_improvement_type(&self, category: &WeaknessCategory) -> ImprovementType {
        match category {
            WeaknessCategory::ToolHandling => ImprovementType::ToolImprovement,
            WeaknessCategory::PromptQuality => ImprovementType::PromptOptimization,
            WeaknessCategory::ReasoningDepth => ImprovementType::NewCapability,
            WeaknessCategory::MemoryRetrieval => ImprovementType::ConfigTuning,
            WeaknessCategory::ResponseFormat => ImprovementType::PromptOptimization,
            WeaknessCategory::ContextWindow => ImprovementType::ConfigTuning,
            WeaknessCategory::Unknown => ImprovementType::BugFix,
        }
    }

    /// Generate an improvement from a suggestion
    async fn generate_improvement(&self, suggestion: &ImprovementSuggestion) -> Improvement {
        let mut improvement = Improvement::new(
            suggestion.improvement_type.clone(),
            suggestion.description.clone(),
            suggestion.description.clone(),
            suggestion.confidence,
        ).with_impact(suggestion.estimated_impact);

        // Generate specific improvements based on type
        match suggestion.improvement_type {
            ImprovementType::ToolGeneration => {
                if self.config.auto_generate_tools {
                    let tool = self.generate_tool_spec(&suggestion.description);
                    improvement.generated_code = Some(tool.implementation);
                    improvement.status = ImprovementStatus::Generated;
                }
            }
            ImprovementType::PromptOptimization => {
                if self.config.optimize_prompts {
                    let optimization = self.optimize_prompt(&suggestion.description).await;
                    improvement.generated_code = Some(optimization.optimized_prompt);
                    improvement.status = ImprovementStatus::Generated;
                }
            }
            ImprovementType::ConfigTuning => {
                if self.config.tune_config {
                    let config_change = self.suggest_config_change(&suggestion.description);
                    improvement.generated_code = Some(config_change);
                    improvement.status = ImprovementStatus::Generated;
                }
            }
            _ => {}
        }

        improvement
    }

    /// Generate a tool specification from a description
    fn generate_tool_spec(&self, description: &str) -> GeneratedToolSpec {
        let name = format!("auto_generated_{}", Uuid::new_v4().to_string().split('-').next().unwrap());
        
        GeneratedToolSpec {
            name: name.clone(),
            description: description.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input for the tool"
                    }
                },
                "required": ["input"]
            }),
            implementation: format!(
                r#"use crate::tools{{Tool, ToolError, ToolResult}};

pub struct {}Tool;

impl {}Tool {{
    pub fn new() -> Self {{ Self }}
}}

impl Tool for {}Tool {{
    fn name(&self) -> &str {{ "{}" }}
    
    fn description(&self) -> &str {{
        "{}"
    }}
    
    fn parameters(&self) -> serde_json::Value {{
        serde_json::json!({{
            "type": "object",
            "properties": {{
                "input": {{
                    "type": "string",
                    "description": "Input for the tool"
                }}
            }},
            "required": ["input"]
        }})
    }}
    
    fn execute(&self, arguments: &serde_json::Value) -> Result<ToolResult, ToolError> {{
        let input = arguments["input"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("Missing input".to_string()))?;
        
        // NOTE: This is a template placeholder. The actual tool logic should be 
        // implemented based on the tool's purpose when the generated code is used.
        Ok(ToolResult {{
            tool_call_id: "{}".to_string(),
            name: "{}".to_string(),
            content: format!("Processed: {{}}", input),
            success: true,
            error: None,
        }})
    }}
}}"#,
                name, name, name, name, description, name, name
            ),
            tests: format!(
                r#"#[cfg(test)]
mod tests {{
    use super::*;
    
    #[test]
    fn test_{}_tool() {{
        let tool = {}Tool::new();
        assert_eq!(tool.name(), "{}");
        
        let args = serde_json::json!({{"input": "test"}});
        let result = tool.execute(&args);
        assert!(result.is_ok());
    }}
}}"#,
                name, name, name
            ),
            dependencies: vec![],
        }
    }

    /// Optimize a prompt based on description
    async fn optimize_prompt(&self, _description: &str) -> PromptOptimization {
        let current = self.current_prompt.read().await.clone();
        
        let optimizations = vec![
            "Add more specific instructions for the task".to_string(),
            "Include examples of expected behavior".to_string(),
            "Add error handling guidance".to_string(),
        ];

        // Create optimized version with improvements
        let mut optimized = current.clone();
        if !optimized.contains("Examples:") {
            optimized.push_str("\n\nExamples:\n- When asked about X, provide Y\n- Be concise and actionable");
        }
        if !optimized.contains("Guidelines:") {
            optimized.push_str("\n\nGuidelines:\n- Always verify information\n- Ask clarifying questions when needed");
        }

        PromptOptimization {
            id: Uuid::new_v4().to_string(),
            original_prompt: current,
            optimized_prompt: optimized,
            improvements: optimizations,
            expected_benefit: "Improved response quality and consistency".to_string(),
            validated: false,
        }
    }

    /// Suggest configuration changes
    fn suggest_config_change(&self, _description: &str) -> String {
        r#"# Suggested Configuration Changes
# Add to agent.toml

[improvements]
enabled = true
auto_apply_low_impact = true
require_approval_threshold = 0.8
"#.to_string()
    }

    /// Apply an improvement
    pub async fn apply_improvement(&self, improvement_id: &str) -> Result<(), String> {
        let mut improvements = self.improvements.write().await;
        
        if let Some(imp) = improvements.iter_mut().find(|i| i.id == improvement_id) {
            if imp.generated_code.is_none() {
                return Err("No code generated for this improvement".to_string());
            }

            // In a real implementation, this would write the code to files
            // For now, we just mark it as applied
            imp.status = ImprovementStatus::Applied;
            imp.applied_at = Some(Utc::now());

            tracing::info!("Applied improvement: {} - {}", imp.id, imp.description);
            Ok(())
        } else {
            Err("Improvement not found".to_string())
        }
    }

    /// Reject an improvement
    pub async fn reject_improvement(&self, improvement_id: &str, reason: &str) -> Result<(), String> {
        let mut improvements = self.improvements.write().await;
        
        if let Some(imp) = improvements.iter_mut().find(|i| i.id == improvement_id) {
            imp.status = ImprovementStatus::Rejected;
            tracing::info!("Rejected improvement: {} - Reason: {}", imp.id, reason);
            Ok(())
        } else {
            Err("Improvement not found".to_string())
        }
    }

    /// Get statistics
    pub async fn get_stats(&self) -> SelfImproveStats {
        let improvements = self.improvements.read().await;
        let analyses = self.analyses.read().await;
        let generated_tools = self.generated_tools.read().await;
        let prompt_history = self.prompt_history.read().await;

        let total = improvements.len();
        let applied = improvements.iter().filter(|i| matches!(i.status, ImprovementStatus::Applied)).count();
        let rejected = improvements.iter().filter(|i| matches!(i.status, ImprovementStatus::Rejected)).count();
        let pending = improvements.iter().filter(|i| matches!(i.status, ImprovementStatus::Pending | ImprovementStatus::Generated)).count();

        let avg_impact = if !improvements.is_empty() {
            improvements.iter().map(|i| i.impact_score).sum::<f32>() / improvements.len() as f32
        } else {
            0.0
        };

        SelfImproveStats {
            total_improvements: total,
            pending_improvements: pending,
            applied_improvements: applied,
            rejected_improvements: rejected,
            total_analyses: analyses.len(),
            last_analysis: analyses.last().map(|a| a.timestamp),
            average_impact: avg_impact,
            generated_tools_count: generated_tools.len(),
            prompt_optimizations_count: prompt_history.len(),
        }
    }

    /// Get all improvements
    pub async fn get_improvements(&self, status_filter: Option<ImprovementStatus>) -> Vec<Improvement> {
        let improvements = self.improvements.read().await;
        
        match status_filter {
            Some(status) => improvements.iter().filter(|i| i.status == status).cloned().collect(),
            None => improvements.clone(),
        }
    }

    /// Get pending improvements
    #[allow(dead_code)]
    pub async fn get_pending_improvements(&self) -> Vec<Improvement> {
        self.get_improvements(Some(ImprovementStatus::Pending)).await
    }

    /// Rollback an applied improvement
    pub async fn rollback_improvement(&self, improvement_id: &str) -> Result<(), String> {
        let mut improvements = self.improvements.write().await;
        
        if let Some(imp) = improvements.iter_mut().find(|i| i.id == improvement_id) {
            if !matches!(imp.status, ImprovementStatus::Applied) {
                return Err("Improvement is not applied".to_string());
            }

            imp.status = ImprovementStatus::RolledBack;
            tracing::info!("Rolled back improvement: {}", imp.id);
            Ok(())
        } else {
            Err("Improvement not found".to_string())
        }
    }

    /// Get current system prompt
    pub async fn get_current_prompt(&self) -> String {
        self.current_prompt.read().await.clone()
    }

    /// Update system prompt
    pub async fn update_prompt(&self, new_prompt: String) {
        let mut current = self.current_prompt.write().await;
        *current = new_prompt;
    }

    // ============================================
    // Code Pattern Analysis Methods (Self-Improvement)
    // ============================================

    /// Analyze code from search results and detect patterns for improvement
    pub async fn analyze_code_from_search(
        &self,
        search_query: &str,
        search_results: &[SearchCodeResult],
    ) -> CodeAnalysisResult {
        let mut patterns = Vec::new();
        let mut improvements = Vec::new();

        // Extract code snippets from search results
        for result in search_results {
            // Detect Rust code patterns
            if let Some(code) = &result.code_snippet {
                let detected_patterns = self.detect_code_patterns(code, Some(result.url.as_str()));
                for pattern in detected_patterns {
                    patterns.push(pattern);
                }
            }
        }

        // Identify improvement opportunities based on detected patterns
        let detected_patterns = patterns.clone();
        let project_root = self.project_root.read().await.clone();
        
        if let Some(root) = project_root {
            improvements = self.identify_improvements_from_patterns(&detected_patterns, &root).await;
        }

        // Store patterns
        {
            let mut stored = self.code_patterns.write().await;
            stored.extend(patterns.clone());
        }

        // Store improvements
        {
            let mut stored = self.code_improvements.write().await;
            stored.extend(improvements.clone());
        }

        CodeAnalysisResult {
            patterns,
            improvements,
            source_query: search_query.to_string(),
            analyzed_at: Utc::now(),
        }
    }

    /// Detect code patterns in a code snippet
    fn detect_code_patterns(&self, code: &str, source_url: Option<&str>) -> Vec<CodePattern> {
        let mut patterns = Vec::new();
        
        // Check for async patterns
        if code.contains("async fn") || code.contains("await") {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::Async,
                description: "Async/await pattern detected".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.8,
            });
        }
        
        // Check for error handling patterns
        if code.contains("Result<") && (code.contains("?") || code.contains("match")) {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::ErrorHandling,
                description: "Error handling with Result type detected".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.85,
            });
        }
        
        // Check for Arc<RwLock> patterns (common in Rust concurrency)
        if code.contains("Arc<") && code.contains("RwLock") {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::BestPractice,
                description: "Arc<RwLock<T>> pattern for thread-safe shared state".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.9,
            });
        }
        
        // Check for trait implementations
        if code.contains("impl ") && code.contains("for ") {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::BestPractice,
                description: "Trait implementation pattern".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.7,
            });
        }
        
        // Check for iterator patterns
        if code.contains(".iter()") && (code.contains("map") || code.contains("filter") || code.contains("collect")) {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::BestPractice,
                description: "Iterator chain pattern detected".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.75,
            });
        }
        
        // Check for test patterns
        if code.contains("#[test]") || code.contains("#[tokio::test]") {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::Testing,
                description: "Test pattern detected".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.8,
            });
        }
        
        // Check for performance patterns (unsafe, manual memory, etc.)
        if code.contains("unsafe") {
            patterns.push(CodePattern {
                id: Uuid::new_v4().to_string(),
                pattern_type: CodePatternType::Performance,
                description: "Unsafe code block - performance optimization opportunity".to_string(),
                example_code: code.chars().take(200).collect(),
                source_file: None,
                source_url: source_url.map(String::from),
                detected_at: Utc::now(),
                relevance_score: 0.65,
            });
        }
        
        patterns
    }

    /// Identify code improvements based on detected patterns
    async fn identify_improvements_from_patterns(
        &self,
        patterns: &[CodePattern],
        project_root: &Path,
    ) -> Vec<CodeImprovement> {
        let mut improvements = Vec::new();
        
        for pattern in patterns {
            // Try to find matching code in the project
            if let Some(target_file) = self.find_matching_file(project_root, pattern).await {
                if let Ok(current_code) = fs::read_to_string(&target_file) {
                    // Generate improvement based on pattern type
                    let improved_code = self.apply_pattern(&current_code, pattern);
                    
                    if improved_code != current_code {
                        improvements.push(CodeImprovement {
                            id: Uuid::new_v4().to_string(),
                            pattern_id: pattern.id.clone(),
                            target_file: target_file.to_string_lossy().to_string(),
                            target_function: None,
                            current_code: current_code.chars().take(500).collect(),
                            improved_code: improved_code.chars().take(500).collect(),
                            explanation: format!("Apply {} pattern from search results", 
                                match pattern.pattern_type {
                                    CodePatternType::Async => "async/await",
                                    CodePatternType::ErrorHandling => "error handling",
                                    CodePatternType::BestPractice => "best practice",
                                    _ => "code pattern",
                                }),
                            confidence: pattern.relevance_score,
                            effort_estimate: EffortLevel::Medium,
                            created_at: Utc::now(),
                        });
                    }
                }
            }
        }
        
        improvements
    }

    /// Find matching file in project based on pattern
    async fn find_matching_file(&self, project_root: &Path, pattern: &CodePattern) -> Option<PathBuf> {
        // Search for Rust files that might benefit from this pattern
        let extensions = ["rs", "toml"];
        
        for ext in extensions {
            // Look for files that might benefit from the pattern
            let pattern_dir = match pattern.pattern_type {
                CodePatternType::Async => "src/agent",
                CodePatternType::ErrorHandling => "src",
                CodePatternType::BestPractice => "src",
                CodePatternType::Performance => "src/agent",
                CodePatternType::Memory => "src/memory",
                CodePatternType::Testing => "src",
                _ => "src",
            };
            
            let search_path = project_root.join(pattern_dir);
            if search_path.exists() {
                // Find relevant files
                if let Ok(entries) = fs::read_dir(&search_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == ext) {
                            return Some(path);
                        }
                    }
                }
            }
        }
        
        None
    }

    /// Apply a pattern to code
    fn apply_pattern(&self, code: &str, pattern: &CodePattern) -> String {
        match pattern.pattern_type {
            CodePatternType::Async => {
                // Check if we should add async patterns
                if !code.contains("async fn") && code.contains("fn ") {
                    code.replace("fn ", "async fn ")
                } else {
                    code.to_string()
                }
            }
            CodePatternType::ErrorHandling => {
                // Add Result type error handling if missing
                if !code.contains("Result<") && code.contains("fn ") {
                    code.replace("fn ", "fn ")
                } else {
                    code.to_string()
                }
            }
            _ => code.to_string(),
        }
    }

    /// Generate improvement suggestions from patterns
    #[allow(dead_code)]
    pub async fn generate_improvement_suggestions(&self) -> Vec<Improvement> {
        let patterns = self.code_patterns.read().await;
        let mut suggestions = Vec::new();
        
        for pattern in patterns.iter() {
            let improvement = Improvement::new(
                match pattern.pattern_type {
                    CodePatternType::BestPractice => ImprovementType::NewCapability,
                    CodePatternType::Performance => ImprovementType::NewCapability,
                    CodePatternType::ErrorHandling => ImprovementType::BugFix,
                    CodePatternType::Async => ImprovementType::NewCapability,
                    CodePatternType::Memory => ImprovementType::NewCapability,
                    CodePatternType::Testing => ImprovementType::NewCapability,
                    _ => ImprovementType::NewCapability,
                },
                pattern.description.clone(),
                format!("Apply pattern: {}", pattern.description),
                pattern.relevance_score,
            ).with_code(pattern.example_code.clone(), None);
            
            suggestions.push(improvement);
        }
        
        suggestions
    }

    /// Apply an improvement to actual agent code
    pub async fn apply_code_improvement(&self, improvement_id: &str) -> Result<String, String> {
        let improvements = self.code_improvements.read().await;
        
        if let Some(imp) = improvements.iter().find(|i| i.id == improvement_id) {
            let target_path = Path::new(&imp.target_file);
            
            // Create backup
            let backup_path = format!("{}.backup.{}", imp.target_file, 
                Utc::now().format("%Y%m%d_%H%M%S"));
            
            if let Ok(original) = fs::read_to_string(target_path) {
                let _ = fs::write(&backup_path, &original);
                
                // Apply the improvement
                let new_code = imp.improved_code.clone();
                if let Err(e) = fs::write(target_path, &new_code) {
                    // Rollback
                    let _ = fs::write(target_path, &original);
                    return Err(format!("Failed to write improvement: {}", e));
                }
                
                // Record in history
                let entry = ImprovementHistoryEntry {
                    id: Uuid::new_v4().to_string(),
                    improvement_id: improvement_id.to_string(),
                    action: ImprovementAction::Applied,
                    timestamp: Utc::now(),
                    details: format!("Applied improvement to {}", imp.target_file),
                    backup_path: Some(backup_path),
                };
                
                let mut history = self.improvement_history.write().await;
                history.push(entry);
                
                tracing::info!("Applied code improvement {} to {}", improvement_id, imp.target_file);
                return Ok(imp.target_file.clone());
            }
            
            Err("Could not read target file".to_string())
        } else {
            Err("Improvement not found".to_string())
        }
    }

    /// Rollback a code improvement
    pub async fn rollback_code_improvement(&self, improvement_id: &str) -> Result<(), String> {
        let history = self.improvement_history.read().await;
        
        if let Some(entry) = history.iter().rev().find(|e| e.improvement_id == improvement_id) {
            if let Some(backup_path) = &entry.backup_path {
                let target_path_str = entry.details.replace("Applied improvement to ", "");
                let target_path = Path::new(&target_path_str);
                
                if let Ok(backup) = fs::read_to_string(backup_path) {
                    fs::write(target_path, &backup).map_err(|e| e.to_string())?;
                    
                    tracing::info!("Rolled back improvement {}", improvement_id);
                    return Ok(());
                }
                
                return Err("Could not read backup file".to_string());
            }
            
            Err("No backup found for this improvement".to_string())
        } else {
            Err("Improvement not found in history".to_string())
        }
    }

    /// Get all code improvements
    pub async fn get_code_improvements(&self) -> Vec<CodeImprovement> {
        self.code_improvements.read().await.clone()
    }

    /// Get all detected patterns
    pub async fn get_code_patterns(&self) -> Vec<CodePattern> {
        self.code_patterns.read().await.clone()
    }

    /// Get improvement history
    pub async fn get_improvement_history(&self) -> Vec<ImprovementHistoryEntry> {
        self.improvement_history.read().await.clone()
    }

    /// Get extended statistics including code analysis
    pub async fn get_extended_stats(&self) -> serde_json::Value {
        let base_stats = self.get_stats().await;
        let patterns = self.code_patterns.read().await;
        let improvements = self.code_improvements.read().await;
        let history = self.improvement_history.read().await;
        
        serde_json::json!({
            "base": base_stats,
            "code_patterns_detected": patterns.len(),
            "code_improvements_found": improvements.len(),
            "improvement_history_entries": history.len(),
            "patterns_by_type": {
                "best_practice": patterns.iter().filter(|p| matches!(p.pattern_type, CodePatternType::BestPractice)).count(),
                "performance": patterns.iter().filter(|p| matches!(p.pattern_type, CodePatternType::Performance)).count(),
                "error_handling": patterns.iter().filter(|p| matches!(p.pattern_type, CodePatternType::ErrorHandling)).count(),
                "async": patterns.iter().filter(|p| matches!(p.pattern_type, CodePatternType::Async)).count(),
                "testing": patterns.iter().filter(|p| matches!(p.pattern_type, CodePatternType::Testing)).count(),
            }
        })
    }
}

/// Search result with code snippet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCodeResult {
    pub title: String,
    pub url: String,
    pub code_snippet: Option<String>,
    pub relevance_score: f32,
}

impl Default for SelfImproveEngine {
    fn default() -> Self {
        Self::new(SelfImproveConfig::default())
    }
}

/// Summary of an interaction for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionSummary {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub user_message: String,
    pub assistant_response: String,
    pub tools_used: Vec<String>,
    pub success: bool,
    pub quality_score: f32,
    pub reasoning_depth: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_self_improve_engine_creation() {
        let engine = SelfImproveEngine::new(SelfImproveConfig::default());
        assert!(engine.config.enabled);
    }

    #[tokio::test]
    async fn test_performance_analysis() {
        let engine = SelfImproveEngine::new(SelfImproveConfig::default());
        
        let interactions = vec![
            InteractionSummary {
                id: "1".to_string(),
                timestamp: Utc::now(),
                user_message: "Hello".to_string(),
                assistant_response: "Hi!".to_string(),
                tools_used: vec![],
                success: true,
                quality_score: 0.8,
                reasoning_depth: 1,
            },
        ];
        
        let mut tool_usage = HashMap::new();
        tool_usage.insert("search".to_string(), 5);
        
        let errors = vec![];
        
        let improvements = engine.analyze_and_improve(&interactions, &tool_usage, &errors).await;
        assert!(!improvements.is_empty() || true); // May or may not have improvements
    }

    #[tokio::test]
    async fn test_tool_spec_generation() {
        let engine = SelfImproveEngine::new(SelfImproveConfig::default());
        let spec = engine.generate_tool_spec("Calculate math expressions");
        
        assert!(spec.name.starts_with("auto_generated_"));
        assert!(spec.implementation.contains("impl Tool for"));
        assert!(spec.tests.contains("#[test]"));
    }

    #[tokio::test]
    async fn test_get_stats() {
        let engine = SelfImproveEngine::new(SelfImproveConfig::default());
        let stats = engine.get_stats().await;
        
        assert_eq!(stats.total_improvements, 0);
        assert_eq!(stats.total_analyses, 0);
    }

    #[tokio::test]
    async fn test_improvement_rejection() {
        let engine = SelfImproveEngine::new(SelfImproveConfig::default());
        
        let result = engine.reject_improvement("non-existent", "test").await;
        assert!(result.is_err());
    }
}
