//! Background services for AGI Agent
//! 
//! Implements background services as specified in SPEC.md:
//! - Session Review Service
//! - Memory Eviction Service
//! - Search Learning Service
//! - Curiosity Engine (Phase 3)
//! - Online Learning Service (Phase 3)
//! - Self-Improvement Engine (Phase 3)
//! - Theory of Mind Engine (Phase 3)

pub mod session_review;
pub mod memory_eviction;
pub mod search_learning;
pub mod curiosity;
pub mod online_learning;
pub mod self_improve;
pub mod theory_of_mind;
pub mod batch_training;

pub use session_review::SessionReviewService;
pub use memory_eviction::MemoryEvictionService;
pub use search_learning::SearchLearningService;
pub use curiosity::CuriosityEngine;
pub use online_learning::OnlineLearningService;
pub use self_improve::{
    SelfImproveEngine, SelfImproveStats,
};
#[allow(unused)]
pub use theory_of_mind::{
    TheoryOfMindEngine, ToMStats, UserMentalState,
    ToMAnalysis, MessageContext,
};
pub use batch_training::BatchTrainingService;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Service status
#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Error(String),
}

impl Default for ServiceStatus {
    fn default() -> Self {
        ServiceStatus::Stopped
    }
}

/// Service manager for running background services
pub struct ServiceManager {
    #[allow(dead_code)]
    session_review: Arc<RwLock<Option<SessionReviewService>>>,
    #[allow(dead_code)]
    memory_eviction: Arc<RwLock<Option<MemoryEvictionService>>>,
    #[allow(dead_code)]
    search_learning: Arc<RwLock<Option<SearchLearningService>>>,
    #[allow(dead_code)]
    curiosity_engine: Arc<RwLock<Option<CuriosityEngine>>>,
    #[allow(dead_code)]
    online_learning: Arc<RwLock<Option<OnlineLearningService>>>,
}

impl ServiceManager {
    pub fn new() -> Self {
        Self {
            session_review: Arc::new(RwLock::new(None)),
            memory_eviction: Arc::new(RwLock::new(None)),
            search_learning: Arc::new(RwLock::new(None)),
            curiosity_engine: Arc::new(RwLock::new(None)),
            online_learning: Arc::new(RwLock::new(None)),
        }
    }

    /// Start all enabled services
    pub async fn start_all(&self) {
        // Session review service
        {
            let mut service = self.session_review.write().await;
            *service = Some(SessionReviewService::new());
        }
        
        // Memory eviction service  
        {
            let mut service = self.memory_eviction.write().await;
            *service = Some(MemoryEvictionService::new());
        }
        
        // Search learning service
        {
            let mut service = self.search_learning.write().await;
            *service = Some(SearchLearningService::new());
        }
        
        // Curiosity engine (Phase 3)
        {
            let mut service = self.curiosity_engine.write().await;
            *service = Some(CuriosityEngine::new());
        }
        
        // Online learning service (Phase 3)
        {
            let mut service = self.online_learning.write().await;
            *service = Some(OnlineLearningService::new());
        }
        
        tracing::info!("All background services started");
    }

    /// Stop all services
    pub async fn stop_all(&self) {
        {
            let mut service = self.session_review.write().await;
            *service = None;
        }
        {
            let mut service = self.memory_eviction.write().await;
            *service = None;
        }
        {
            let mut service = self.search_learning.write().await;
            *service = None;
        }
        {
            let mut service = self.curiosity_engine.write().await;
            *service = None;
        }
        {
            let mut service = self.online_learning.write().await;
            *service = None;
        }
        
        tracing::info!("All background services stopped");
    }

    /// Get session review service
    pub fn session_review(&self) -> Option<Arc<SessionReviewService>> {
        self.session_review.blocking_read().as_ref().cloned().map(Arc::new)
    }

    /// Get memory eviction service
    pub fn memory_eviction(&self) -> Option<Arc<MemoryEvictionService>> {
        self.memory_eviction.blocking_read().as_ref().cloned().map(Arc::new)
    }

    /// Get search learning service
    pub fn search_learning(&self) -> Option<Arc<SearchLearningService>> {
        self.search_learning.blocking_read().as_ref().cloned().map(Arc::new)
    }

    /// Get curiosity engine
    pub fn curiosity_engine(&self) -> Option<Arc<CuriosityEngine>> {
        self.curiosity_engine.blocking_read().as_ref().cloned().map(Arc::new)
    }

    /// Get online learning service
    pub fn online_learning(&self) -> Option<Arc<OnlineLearningService>> {
        self.online_learning.blocking_read().as_ref().cloned().map(Arc::new)
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}
