use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::{SharedSession, DirectoryStatus};
use super::task_manager::{Task, TaskMetrics};
use crate::utils::config::Config;
use tracing::{debug, error};

/// Trait for processing backup tasks
/// This trait helps break circular dependencies between task_manager and worker
#[async_trait]
pub trait TaskProcessor: Send + Sync {
    async fn process_task(
        &self,
        worker_id: usize,
        task: &Task,
        session: &SharedSession,
        config: &Arc<Config>,
    ) -> Result<TaskMetrics>;
}

/// Default implementation of TaskProcessor that uses BackupWorker
pub struct BackupTaskProcessor {
    event_tx: broadcast::Sender<super::manager::Event>,
    log_buffer: Option<crate::utils::log_buffer::LogBuffer>,
}

impl BackupTaskProcessor {
    pub fn new(event_tx: broadcast::Sender<super::manager::Event>) -> Self {
        Self { 
            event_tx,
            log_buffer: None,
        }
    }
    
    pub fn with_log_buffer(event_tx: broadcast::Sender<super::manager::Event>, log_buffer: crate::utils::log_buffer::LogBuffer) -> Self {
        Self {
            event_tx,
            log_buffer: Some(log_buffer),
        }
    }
}

#[async_trait]
impl TaskProcessor for BackupTaskProcessor {
    async fn process_task(
        &self,
        worker_id: usize,
        task: &Task,
        session: &SharedSession,
        config: &Arc<Config>,
    ) -> Result<TaskMetrics> {
        // Get directory info
        let (name, path) = {
            let session = session.read().unwrap();
            let dir = &session.directories[task.directory_index];
            (dir.name.clone(), dir.path.clone())
        };
        
        debug!("Worker {} processing directory: {} at {:?}", worker_id, name, path);
        
        // Create a backup worker to handle the actual rsync
        let worker = crate::backup::BackupWorker::new(
            worker_id,
            session.clone(),
            self.event_tx.clone(),
            config.clone(),
            self.log_buffer.clone(),
        );
        
        // Mark directory as active
        {
            let mut session_guard = session.write().unwrap();
            if let Some(dir) = session_guard.directories.get_mut(task.directory_index) {
                dir.status = DirectoryStatus::Active;
                dir.start_time = Some(chrono::Utc::now().timestamp());
            }
        }
        
        // Run the backup
        let start_time = std::time::Instant::now();
        match worker.process_single_directory(task.directory_index).await {
            Ok(()) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let session_guard = session.read().unwrap();
                let dir = &session_guard.directories[task.directory_index];
                
                let mut metrics = TaskMetrics::default();
                metrics.bytes_processed = dir.size_copied;
                metrics.files_processed = dir.files_processed;
                metrics.duration_ms = duration_ms;
                metrics.average_speed_mbps = if duration_ms > 0 {
                    (dir.size_copied as f64 / 1_048_576.0) / (duration_ms as f64 / 1000.0)
                } else {
                    0.0
                };
                
                Ok(metrics)
            }
            Err(e) => {
                error!("Worker {} failed on directory {}: {}", worker_id, name, e);
                Err(e)
            }
        }
    }
}