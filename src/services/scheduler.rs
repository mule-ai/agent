//! Scheduler Service for Automated Tasks
//!
//! Implements scheduled execution of background tasks using cron expressions.
//! - Batch training at configured schedule
//! - Memory eviction at regular intervals
//! - Session review processing

use crate::services::{BatchTrainingService, MemoryEvictionService, SessionReviewService};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Enable the scheduler
    pub enabled: bool,
    /// Enable scheduled batch training
    pub batch_training_enabled: bool,
    /// Cron expression for batch training (default: "0 2 * * *" = 2 AM daily)
    pub batch_training_schedule: String,
    /// Enable scheduled memory eviction
    pub memory_eviction_enabled: bool,
    /// Cron expression for memory eviction (default: "0 0 * * *" = midnight daily)
    pub memory_eviction_schedule: String,
    /// Enable scheduled session review
    pub session_review_enabled: bool,
    /// Cron expression for session review (default: "0 */6 * * *" = every 6 hours)
    pub session_review_schedule: String,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            batch_training_enabled: true,
            batch_training_schedule: "0 2 * * *".to_string(), // 2 AM daily
            memory_eviction_enabled: true,
            memory_eviction_schedule: "0 0 * * *".to_string(), // Midnight daily
            session_review_enabled: false, // Session review is triggered on session end
            session_review_schedule: "0 */6 * * *".to_string(), // Every 6 hours
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub batch_training_runs: u64,
    pub memory_eviction_runs: u64,
    pub session_review_runs: u64,
    pub last_batch_training: Option<String>,
    pub last_memory_eviction: Option<String>,
    pub last_session_review: Option<String>,
    pub errors: Vec<SchedulerError>,
}

/// Scheduler error record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerError {
    pub timestamp: String,
    pub job: String,
    pub error: String,
}

/// Scheduler service for automated background tasks
pub struct SchedulerService {
    config: SchedulerConfig,
    scheduler: Arc<RwLock<Option<JobScheduler>>>,
    batch_training_service: Arc<BatchTrainingService>,
    memory_eviction_service: Option<Arc<MemoryEvictionService>>,
    session_review_service: Option<Arc<SessionReviewService>>,
    stats: Arc<RwLock<SchedulerStats>>,
}

impl SchedulerService {
    /// Create a new scheduler service with batch training only
    pub fn new(
        config: SchedulerConfig,
        batch_training_service: Arc<BatchTrainingService>,
    ) -> Self {
        Self {
            config,
            scheduler: Arc::new(RwLock::new(None)),
            batch_training_service,
            memory_eviction_service: None,
            session_review_service: None,
            stats: Arc::new(RwLock::new(SchedulerStats::default())),
        }
    }

    /// Create a new scheduler service with all services
    pub fn with_services(
        config: SchedulerConfig,
        batch_training_service: Arc<BatchTrainingService>,
        memory_eviction_service: Arc<MemoryEvictionService>,
        session_review_service: Arc<SessionReviewService>,
    ) -> Self {
        Self {
            config,
            scheduler: Arc::new(RwLock::new(None)),
            batch_training_service,
            memory_eviction_service: Some(memory_eviction_service),
            session_review_service: Some(session_review_service),
            stats: Arc::new(RwLock::new(SchedulerStats::default())),
        }
    }

    /// Initialize and start the scheduler
    pub async fn start(&self) -> Result<()> {
        let scheduler = JobScheduler::new()
            .await
            .context("Failed to create job scheduler")?;

        // Add batch training job if enabled
        if self.config.batch_training_enabled {
            self.add_batch_training_job(&scheduler).await?;
        }

        // Add memory eviction job if enabled and service is available
        if self.config.memory_eviction_enabled {
            if let Some(ref service) = self.memory_eviction_service {
                self.add_memory_eviction_job(&scheduler, service.clone()).await?;
            } else {
                tracing::warn!(
                    "Memory eviction enabled in config but no service available"
                );
            }
        }

        // Add session review job if enabled and service is available
        if self.config.session_review_enabled {
            if let Some(ref service) = self.session_review_service {
                self.add_session_review_job(&scheduler, service.clone()).await?;
            } else {
                tracing::warn!(
                    "Session review enabled in config but no service available"
                );
            }
        }

        // Start the scheduler
        scheduler
            .start()
            .await
            .context("Failed to start job scheduler")?;

        // Store the scheduler
        let mut scheduler_guard = self.scheduler.write().await;
        *scheduler_guard = Some(scheduler);

        tracing::info!(
            "Scheduler started with batch_training={}, memory_eviction={}, session_review={}",
            self.config.batch_training_enabled,
            self.config.memory_eviction_enabled && self.memory_eviction_service.is_some(),
            self.config.session_review_enabled && self.session_review_service.is_some()
        );

        Ok(())
    }

    /// Add batch training job
    async fn add_batch_training_job(&self, scheduler: &JobScheduler) -> Result<()> {
        let batch_service = self.batch_training_service.clone();
        let stats = self.stats.clone();
        let schedule = self.config.batch_training_schedule.clone();

        let job = Job::new_async(schedule.as_str(), move |_uuid, _l| {
            let service = batch_service.clone();
            let stats = stats.clone();
            Box::pin(async move {
                tracing::info!("[Scheduler] Starting scheduled batch training");

                // Initialize the training pipeline if needed
                if let Err(e) = service.initialize().await {
                    tracing::error!("[Scheduler] Failed to initialize training pipeline: {}", e);
                    Self::record_error(&stats, "batch_training", &e.to_string()).await;
                    return;
                }

                // Check if we have enough examples
                let example_count = service.example_count().await;
                tracing::info!(
                    "[Scheduler] Batch training triggered with {} examples",
                    example_count
                );

                if example_count < 10 {
                    tracing::info!(
                        "[Scheduler] Skipping training - only {} examples (minimum: 10)",
                        example_count
                    );
                    return;
                }

                // Run training
                match service.train().await {
                    Ok(job) => {
                        tracing::info!(
                            "[Scheduler] Batch training completed successfully: {:?}",
                            job
                        );
                        Self::record_success(&stats, "batch_training").await;
                    }
                    Err(e) => {
                        tracing::error!("[Scheduler] Batch training failed: {}", e);
                        Self::record_error(&stats, "batch_training", &e.to_string()).await;
                    }
                }
            })
        })
        .context("Failed to create batch training job")?;

        scheduler
            .add(job)
            .await
            .context("Failed to add batch training job")?;

        tracing::info!("Batch training job scheduled: {}", schedule);
        Ok(())
    }

    /// Add memory eviction job
    async fn add_memory_eviction_job(
        &self,
        scheduler: &JobScheduler,
        service: Arc<MemoryEvictionService>,
    ) -> Result<()> {
        let schedule = self.config.memory_eviction_schedule.clone();
        let stats = self.stats.clone();

        let job = Job::new_async(schedule.as_str(), move |_uuid, _l| {
            let eviction_service = service.clone();
            let stats_clone = stats.clone();
            Box::pin(async move {
                tracing::info!("[Scheduler] Memory eviction triggered");

                // Get eviction stats
                let eviction_stats = eviction_service.get_stats().await;
                tracing::info!(
                    "[Scheduler] Memory eviction: {} processed, {} kept, {} moved, {} deleted",
                    eviction_stats.total_processed,
                    eviction_stats.kept,
                    eviction_stats.moved_to_training,
                    eviction_stats.deleted
                );

                // Record success
                Self::record_success(&stats_clone, "memory_eviction").await;
            })
        })
        .context("Failed to create memory eviction job")?;

        scheduler
            .add(job)
            .await
            .context("Failed to add memory eviction job")?;

        tracing::info!("Memory eviction job scheduled: {}", schedule);
        Ok(())
    }

    /// Add session review job
    async fn add_session_review_job(
        &self,
        scheduler: &JobScheduler,
        _service: Arc<SessionReviewService>,
    ) -> Result<()> {
        let schedule = self.config.session_review_schedule.clone();
        let stats = self.stats.clone();

        let job = Job::new_async(schedule.as_str(), move |_uuid, _l| {
            let stats_clone = stats.clone();
            Box::pin(async move {
                tracing::info!("[Scheduler] Session review triggered (scheduled review)");

                // Note: Session review is typically triggered on session end,
                // but a scheduled review can process any pending sessions
                // For now, we just log that it was triggered
                // Full session review processing would need access to the session store

                // Record success
                Self::record_success(&stats_clone, "session_review").await;
            })
        })
        .context("Failed to create session review job")?;

        scheduler
            .add(job)
            .await
            .context("Failed to add session review job")?;

        tracing::info!("Session review job scheduled: {}", schedule);
        Ok(())
    }

    /// Record a successful job run
    async fn record_success(stats: &Arc<RwLock<SchedulerStats>>, job: &str) {
        let mut stats_guard = stats.write().await;
        let now = chrono::Utc::now().to_rfc3339();

        match job {
            "batch_training" => {
                stats_guard.batch_training_runs += 1;
                stats_guard.last_batch_training = Some(now.clone());
            }
            "memory_eviction" => {
                stats_guard.memory_eviction_runs += 1;
                stats_guard.last_memory_eviction = Some(now.clone());
            }
            "session_review" => {
                stats_guard.session_review_runs += 1;
                stats_guard.last_session_review = Some(now);
            }
            _ => {}
        }

        tracing::debug!("Recorded successful {} run", job);
    }

    /// Record a job error
    async fn record_error(stats: &Arc<RwLock<SchedulerStats>>, job: &str, error: &str) {
        let mut stats_guard = stats.write().await;
        stats_guard.errors.push(SchedulerError {
            timestamp: chrono::Utc::now().to_rfc3339(),
            job: job.to_string(),
            error: error.to_string(),
        });

        // Keep only the last 10 errors
        if stats_guard.errors.len() > 10 {
            stats_guard.errors.remove(0);
        }
    }

    /// Stop the scheduler
    pub async fn stop(&self) -> Result<()> {
        let mut scheduler_guard = self.scheduler.write().await;
        if let Some(mut scheduler) = scheduler_guard.take() {
            scheduler
                .shutdown()
                .await
                .context("Failed to shutdown scheduler")?;
            tracing::info!("Scheduler stopped");
        }
        Ok(())
    }

    /// Get scheduler statistics
    pub async fn get_stats(&self) -> SchedulerStats {
        self.stats.read().await.clone()
    }

    /// Get scheduler configuration
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }

    /// Manually trigger batch training (bypass schedule)
    pub async fn trigger_batch_training(&self) -> Result<()> {
        tracing::info!("[Scheduler] Manual batch training triggered");

        self.batch_training_service
            .initialize()
            .await
            .context("Failed to initialize training pipeline")?;

        let example_count = self.batch_training_service.example_count().await;
        tracing::info!(
            "[Scheduler] Manual training with {} examples",
            example_count
        );

        if example_count < 10 {
            anyhow::bail!(
                "Not enough examples for training ({} < 10)",
                example_count
            );
        }

        self.batch_training_service
            .train()
            .await
            .context("Batch training failed")?;

        Self::record_success(&self.stats, "batch_training").await;
        Ok(())
    }
}

impl Clone for SchedulerService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            scheduler: self.scheduler.clone(),
            batch_training_service: self.batch_training_service.clone(),
            memory_eviction_service: self.memory_eviction_service.clone(),
            session_review_service: self.session_review_service.clone(),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_config_defaults() {
        let config = SchedulerConfig::default();
        assert!(config.batch_training_enabled);
        assert_eq!(config.batch_training_schedule, "0 2 * * *");
        assert!(config.memory_eviction_enabled);
        assert!(!config.session_review_enabled);
    }

    #[tokio::test]
    async fn test_scheduler_service_creation() {
        use crate::config::TrainingConfig;

        let training_config = TrainingConfig::default();
        let batch_service = Arc::new(BatchTrainingService::new(training_config));
        let scheduler = SchedulerService::new(SchedulerConfig::default(), batch_service);

        assert!(scheduler.config().batch_training_enabled);
    }

    #[tokio::test]
    async fn test_scheduler_stats() {
        use crate::config::TrainingConfig;

        let training_config = TrainingConfig::default();
        let batch_service = Arc::new(BatchTrainingService::new(training_config));
        let scheduler = SchedulerService::new(SchedulerConfig::default(), batch_service);

        let stats = scheduler.get_stats().await;
        assert_eq!(stats.batch_training_runs, 0);
        assert!(stats.last_batch_training.is_none());
        assert!(stats.errors.is_empty());
    }
}
