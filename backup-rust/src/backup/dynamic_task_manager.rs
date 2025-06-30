use super::task_manager::{TaskId, TaskManager, TaskManagerStatus, WorkItem};
use super::task_processor::TaskProcessor;
use crate::utils::resource_monitor::{ResourceMonitor, WorkerControl, SmartAllocator, WorkloadProfile, BackupType};
use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, info};

/// Dynamic task manager that scales workers based on system resources
pub struct DynamicTaskManager {
    /// Base task manager
    inner: TaskManager,
    
    /// Resource monitor
    resource_monitor: Arc<ResourceMonitor>,
    
    /// Smart allocator
    smart_allocator: SmartAllocator,
    
    /// Current active workers
    active_workers: Arc<AtomicUsize>,
    
    /// Worker pool (can grow/shrink)
    worker_pool: Arc<RwLock<Vec<WorkerHandle>>>,
    
    /// Channels for dynamic worker creation
    work_sender: Sender<WorkItem>,
    work_receiver: Receiver<WorkItem>,
    
    /// Session and config
    session: super::SharedSession,
    config: Arc<crate::utils::config::Config>,
    
    /// Task processor
    task_processor: Option<Arc<dyn TaskProcessor>>,
}

struct WorkerHandle {
    id: usize,
    handle: tokio::task::JoinHandle<()>,
    active: Arc<AtomicUsize>,
}

impl DynamicTaskManager {
    pub fn new(
        resource_monitor: Arc<ResourceMonitor>,
        session: super::SharedSession,
        config: Arc<crate::utils::config::Config>,
    ) -> Self {
        let smart_allocator = SmartAllocator::new(resource_monitor.clone());
        
        // Create channels with larger capacity for dynamic scaling
        let (work_sender, work_receiver) = bounded(256);
        
        Self {
            inner: TaskManager::new(0), // We'll manage workers ourselves
            resource_monitor,
            smart_allocator,
            active_workers: Arc::new(AtomicUsize::new(0)),
            worker_pool: Arc::new(RwLock::new(Vec::new())),
            work_sender,
            work_receiver,
            session,
            config,
            task_processor: None,
        }
    }
    
    /// Set the task processor
    pub fn set_task_processor(&mut self, processor: Arc<dyn TaskProcessor>) {
        self.task_processor = Some(processor.clone());
        self.inner.set_task_processor(processor);
    }
    
    /// Start with dynamic scaling based on workload
    pub async fn start_dynamic(&mut self, workload: WorkloadProfile) -> Result<()> {
        info!("Starting dynamic task manager for workload: {:?}", workload);
        
        // Get smart allocation
        let allocation = self.smart_allocator.allocate_for_workload(&workload);
        info!("Smart allocation: {} workers ({} MB each) - priority: {}", 
            allocation.worker_count, 
            allocation.memory_per_worker_mb,
            allocation.priority_hint
        );
        
        // Start initial workers
        self.add_workers(allocation.worker_count).await?;
        
        // Start resource monitoring for dynamic scaling
        let monitor = self.resource_monitor.clone();
        let controller = Arc::new(self.clone()) as Arc<dyn WorkerControl + Send + Sync>;
        
        tokio::spawn(async move {
            monitor.start_monitoring(controller).await;
        });
        
        Ok(())
    }
    
    /// Add a task with workload-aware scheduling
    pub fn add_task_smart(&self, directory_index: usize, size: u64) -> TaskId {
        // Calculate priority based on size and current load
        let utilization = self.resource_monitor.get_utilization();
        
        let priority = if size > 1_073_741_824 { // > 1GB
            // Large directories get lower priority when system is busy
            if utilization.cpu_usage > 80.0 {
                1
            } else {
                5
            }
        } else {
            // Small directories get higher priority for quick wins
            8
        };
        
        self.inner.add_task(directory_index, priority, size)
    }
    
    /// Spawn a new worker
    async fn spawn_worker(&self, worker_id: usize) -> Result<WorkerHandle> {
        let work_receiver = self.work_receiver.clone();
        // Get result_sender from the task manager
        let result_sender = self.inner.result_sender.clone();
        let task_status = self.inner.task_status.clone();
        let session = self.session.clone();
        let config = self.config.clone();
        let active_counter = self.active_workers.clone();
        
        // Increment active counter
        active_counter.fetch_add(1, Ordering::SeqCst);
        
        let active_clone = active_counter.clone();
        let task_processor = self.task_processor.clone()
            .ok_or_else(|| anyhow::anyhow!("No task processor set"))?;
        
        let handle = tokio::spawn(async move {
            debug!("Worker {} started", worker_id);
            
            // Run worker loop
            super::task_manager::worker_loop(
                worker_id,
                work_receiver,
                result_sender,
                task_status,
                session,
                config,
                task_processor,
            ).await;
            
            // Decrement active counter when done
            active_clone.fetch_sub(1, Ordering::SeqCst);
            debug!("Worker {} stopped", worker_id);
        });
        
        Ok(WorkerHandle {
            id: worker_id,
            handle,
            active: active_counter.clone(),
        })
    }
}

#[async_trait::async_trait]
impl WorkerControl for DynamicTaskManager {
    async fn get_worker_count(&self) -> usize {
        self.active_workers.load(Ordering::SeqCst)
    }
    
    async fn add_workers(&self, count: usize) -> Result<()> {
        let start_id = {
            let pool = self.worker_pool.read().unwrap();
            pool.len()
        };
        
        let mut new_workers = Vec::new();
        for i in 0..count {
            let worker = self.spawn_worker(start_id + i).await?;
            new_workers.push(worker);
        }
        
        {
            let mut pool = self.worker_pool.write().unwrap();
            pool.extend(new_workers);
        }
        
        info!("Added {} workers, total active: {}", 
            count, 
            self.active_workers.load(Ordering::SeqCst)
        );
        
        Ok(())
    }
    
    async fn remove_workers(&self, count: usize) -> Result<()> {
        let mut pool = self.worker_pool.write().unwrap();
        let to_remove = count.min(pool.len());
        
        // Send shutdown signals
        for _ in 0..to_remove {
            let _ = self.work_sender.try_send(WorkItem::Shutdown);
        }
        
        // Remove handles (they'll clean up when they get shutdown signal)
        for _ in 0..to_remove {
            if let Some(worker) = pool.pop() {
                // Don't wait for handle, let it clean up async
                tokio::spawn(async move {
                    let _ = worker.handle.await;
                });
            }
        }
        
        info!("Removing {} workers, active will be: {}", 
            to_remove,
            self.active_workers.load(Ordering::SeqCst) - to_remove
        );
        
        Ok(())
    }
}

impl Clone for DynamicTaskManager {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            resource_monitor: self.resource_monitor.clone(),
            smart_allocator: SmartAllocator::new(self.resource_monitor.clone()),
            active_workers: self.active_workers.clone(),
            worker_pool: self.worker_pool.clone(),
            work_sender: self.work_sender.clone(),
            work_receiver: self.work_receiver.clone(),
            session: self.session.clone(),
            config: self.config.clone(),
            task_processor: self.task_processor.clone(),
        }
    }
}

/// Enhanced status with resource information
#[derive(Debug, Clone)]
pub struct DynamicTaskStatus {
    pub task_status: TaskManagerStatus,
    pub worker_count: usize,
    pub cpu_usage: f32,
    pub memory_usage_mb: u64,
    pub optimal_workers: usize,
    pub scaling_state: ScalingState,
}

#[derive(Debug, Clone)]
pub enum ScalingState {
    Stable,
    ScalingUp { target: usize },
    ScalingDown { target: usize },
}

impl DynamicTaskManager {
    pub fn get_dynamic_status(&self) -> DynamicTaskStatus {
        let task_status = self.inner.get_status();
        let utilization = self.resource_monitor.get_utilization();
        let current_workers = self.active_workers.load(Ordering::SeqCst);
        
        let scaling_state = if utilization.optimal_workers > current_workers {
            ScalingState::ScalingUp { target: utilization.optimal_workers }
        } else if utilization.optimal_workers < current_workers {
            ScalingState::ScalingDown { target: utilization.optimal_workers }
        } else {
            ScalingState::Stable
        };
        
        DynamicTaskStatus {
            task_status,
            worker_count: current_workers,
            cpu_usage: utilization.cpu_usage,
            memory_usage_mb: utilization.memory_used_mb,
            optimal_workers: utilization.optimal_workers,
            scaling_state,
        }
    }
    
    /// Analyze directories and determine backup type
    pub fn analyze_workload(directories: &[super::Directory]) -> WorkloadProfile {
        let total_size: u64 = directories.iter().map(|d| d.size).sum();
        let total_count = directories.len();
        
        // Estimate file count (rough)
        let avg_file_size = 1_048_576; // Assume 1MB average
        let estimated_files = total_size / avg_file_size;
        
        let backup_type = if total_size > 500_000_000_000 { // > 500GB
            BackupType::FullBackup
        } else if estimated_files > 100_000 {
            BackupType::SmallFiles
        } else if total_count < 10 && total_size > 10_000_000_000 { // Few dirs, > 10GB
            BackupType::LargeFiles
        } else {
            BackupType::Incremental
        };
        
        WorkloadProfile {
            backup_type,
            total_size,
            file_count: estimated_files,
            directory_count: total_count,
        }
    }
}