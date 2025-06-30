use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::{debug, info, warn};

use super::task_processor::TaskProcessor;

/// High-performance task manager for coordinating backup operations
/// Uses lock-free structures and zero-copy message passing
pub struct TaskManager {
    /// Task queue - lock-free for readers
    pub task_queue: Arc<RwLock<VecDeque<Task>>>,
    
    /// Worker pool
    pub workers: Vec<WorkerHandle>,
    
    /// Task status tracking - minimal locking
    pub task_status: Arc<RwLock<HashMap<TaskId, TaskStatus>>>,
    
    /// Channel for worker communication (bounded for backpressure)
    pub work_sender: Sender<WorkItem>,
    pub work_receiver: Receiver<WorkItem>,
    
    /// Result channel for completed tasks
    pub result_sender: Sender<TaskResult>,
    pub result_receiver: Receiver<TaskResult>,
    
    /// Performance metrics
    metrics: Arc<Mutex<Metrics>>,
    
    /// Task processor for handling actual work
    task_processor: Option<Arc<dyn TaskProcessor>>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub directory_index: usize,
    pub priority: u8,
    pub estimated_size: u64,
    pub created_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Queued,
    Assigned { worker_id: usize },
    Running { worker_id: usize, progress: u8 },
    Completed { duration_ms: u64, bytes_processed: u64 },
    Failed { error: String },
}

#[derive(Debug)]
pub enum WorkItem {
    Task(Task),
    Shutdown,
}

#[derive(Debug)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub worker_id: usize,
    pub status: TaskStatus,
    pub metrics: TaskMetrics,
}

#[derive(Debug, Default)]
pub struct TaskMetrics {
    pub files_processed: u64,
    pub bytes_processed: u64,
    pub duration_ms: u64,
    pub average_speed_mbps: f64,
}

#[derive(Debug, Default)]
struct Metrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub total_bytes: u64,
    pub total_duration_ms: u64,
}

pub struct WorkerHandle {
    pub id: usize,
    pub handle: tokio::task::JoinHandle<()>,
}

impl TaskManager {
    pub fn new(num_workers: usize) -> Self {
        // Use bounded channels for backpressure control
        let (work_sender, work_receiver) = bounded(num_workers * 2);
        let (result_sender, result_receiver) = bounded(num_workers * 4);
        
        Self {
            task_queue: Arc::new(RwLock::new(VecDeque::new())),
            workers: Vec::with_capacity(num_workers),
            task_status: Arc::new(RwLock::new(HashMap::new())),
            work_sender,
            work_receiver,
            result_sender,
            result_receiver,
            metrics: Arc::new(Mutex::new(Metrics::default())),
            task_processor: None,
        }
    }
    
    /// Set the task processor
    pub fn set_task_processor(&mut self, processor: Arc<dyn TaskProcessor>) {
        self.task_processor = Some(processor);
    }
    
    /// Start the task manager with the specified number of workers
    pub async fn start(
        &mut self,
        num_workers: usize,
        session: super::SharedSession,
        config: Arc<crate::utils::config::Config>,
    ) -> Result<()> {
        info!("Starting task manager with {} workers", num_workers);
        
        // Ensure we have a task processor
        let task_processor = self.task_processor.clone()
            .ok_or_else(|| anyhow::anyhow!("No task processor set"))?;
        
        // Spawn worker tasks
        for worker_id in 0..num_workers {
            let work_receiver = self.work_receiver.clone();
            let result_sender = self.result_sender.clone();
            let task_status = self.task_status.clone();
            let session = session.clone();
            let config = config.clone();
            
            let task_processor_clone = task_processor.clone();
            
            let handle = tokio::spawn(async move {
                worker_loop(
                    worker_id,
                    work_receiver,
                    result_sender,
                    task_status,
                    session,
                    config,
                    task_processor_clone,
                ).await;
            });
            
            self.workers.push(WorkerHandle {
                id: worker_id,
                handle,
            });
        }
        
        // Start the result processor
        let result_receiver = self.result_receiver.clone();
        let metrics = self.metrics.clone();
        let task_status = self.task_status.clone();
        
        tokio::spawn(async move {
            process_results(result_receiver, metrics, task_status).await;
        });
        
        Ok(())
    }
    
    /// Add a task to the queue
    pub fn add_task(&self, directory_index: usize, priority: u8, estimated_size: u64) -> TaskId {
        let task_id = TaskId(uuid::Uuid::new_v4().as_u128() as u64);
        
        let task = Task {
            id: task_id,
            directory_index,
            priority,
            estimated_size,
            created_at: Instant::now(),
        };
        
        // Add to queue (minimal locking)
        {
            let mut queue = self.task_queue.write().unwrap();
            
            // Insert based on priority (higher priority first)
            let insert_pos = queue.iter()
                .position(|t| t.priority < priority)
                .unwrap_or(queue.len());
            
            queue.insert(insert_pos, task.clone());
        }
        
        // Update status
        self.task_status.write().unwrap().insert(task_id, TaskStatus::Queued);
        
        // Try to dispatch immediately
        self.dispatch_next_task();
        
        task_id
    }
    
    /// Dispatch the next task to an available worker
    fn dispatch_next_task(&self) {
        if let Some(task) = self.task_queue.write().unwrap().pop_front() {
            match self.work_sender.try_send(WorkItem::Task(task.clone())) {
                Ok(_) => {
                    debug!("Dispatched task {:?}", task.id);
                }
                Err(_) => {
                    // Put it back if channel is full
                    self.task_queue.write().unwrap().push_front(task);
                }
            }
        }
    }
    
    /// Get current status of all tasks
    pub fn get_status(&self) -> TaskManagerStatus {
        let task_status = self.task_status.read().unwrap();
        let metrics = self.metrics.lock();
        
        let queued = task_status.values()
            .filter(|s| matches!(s, TaskStatus::Queued))
            .count();
        
        let running = task_status.values()
            .filter(|s| matches!(s, TaskStatus::Running { .. }))
            .count();
        
        let completed = task_status.values()
            .filter(|s| matches!(s, TaskStatus::Completed { .. }))
            .count();
        
        TaskManagerStatus {
            queued_tasks: queued,
            running_tasks: running,
            completed_tasks: completed,
            failed_tasks: metrics.tasks_failed as usize,
            total_bytes_processed: metrics.total_bytes,
            average_speed_mbps: if metrics.total_duration_ms > 0 {
                (metrics.total_bytes as f64 / 1_048_576.0) / 
                (metrics.total_duration_ms as f64 / 1000.0)
            } else {
                0.0
            },
        }
    }
    
    /// Shutdown all workers gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down task manager");
        
        // Send shutdown signal to all workers
        for _ in 0..self.workers.len() {
            let _ = self.work_sender.send(WorkItem::Shutdown);
        }
        
        // Wait for workers to finish
        for worker in self.workers.drain(..) {
            let _ = worker.handle.await;
        }
        
        Ok(())
    }
}

impl Clone for TaskManager {
    fn clone(&self) -> Self {
        Self {
            task_queue: self.task_queue.clone(),
            workers: Vec::new(), // Don't clone workers
            task_status: self.task_status.clone(),
            work_sender: self.work_sender.clone(),
            work_receiver: self.work_receiver.clone(),
            result_sender: self.result_sender.clone(),
            result_receiver: self.result_receiver.clone(),
            metrics: self.metrics.clone(),
            task_processor: self.task_processor.clone(),
        }
    }
}

/// Worker loop - runs in separate tokio task
pub async fn worker_loop(
    worker_id: usize,
    work_receiver: Receiver<WorkItem>,
    result_sender: Sender<TaskResult>,
    task_status: Arc<RwLock<HashMap<TaskId, TaskStatus>>>,
    session: super::SharedSession,
    config: Arc<crate::utils::config::Config>,
    task_processor: Arc<dyn TaskProcessor>,
) {
    info!("Worker {} started", worker_id);
    
    loop {
        match work_receiver.recv() {
            Ok(WorkItem::Task(task)) => {
                // Update status to assigned
                task_status.write().unwrap().insert(
                    task.id,
                    TaskStatus::Assigned { worker_id }
                );
                
                // Process the task
                let start = Instant::now();
                let result = task_processor.process_task(
                    worker_id,
                    &task,
                    &session,
                    &config,
                ).await;
                
                let duration_ms = start.elapsed().as_millis() as u64;
                
                // Send result
                let (status, metrics) = match result {
                    Ok(metrics) => (
                        TaskStatus::Completed {
                            duration_ms,
                            bytes_processed: metrics.bytes_processed,
                        },
                        metrics
                    ),
                    Err(e) => (
                        TaskStatus::Failed {
                            error: e.to_string(),
                        },
                        TaskMetrics::default()
                    ),
                };
                
                let _ = result_sender.send(TaskResult {
                    task_id: task.id,
                    worker_id,
                    status,
                    metrics,
                });
            }
            Ok(WorkItem::Shutdown) => {
                info!("Worker {} shutting down", worker_id);
                break;
            }
            Err(_) => {
                warn!("Worker {} channel closed", worker_id);
                break;
            }
        }
    }
}


/// Process results from workers
async fn process_results(
    result_receiver: Receiver<TaskResult>,
    metrics: Arc<Mutex<Metrics>>,
    task_status: Arc<RwLock<HashMap<TaskId, TaskStatus>>>,
) {
    while let Ok(result) = result_receiver.recv() {
        // Update task status
        task_status.write().unwrap().insert(result.task_id, result.status.clone());
        
        // Update metrics
        let mut m = metrics.lock();
        match &result.status {
            TaskStatus::Completed { duration_ms, bytes_processed } => {
                m.tasks_completed += 1;
                m.total_bytes += bytes_processed;
                m.total_duration_ms += duration_ms;
            }
            TaskStatus::Failed { .. } => {
                m.tasks_failed += 1;
            }
            _ => {}
        }
        
        debug!("Task {:?} completed by worker {}", result.task_id, result.worker_id);
    }
}

#[derive(Debug, Clone)]
pub struct TaskManagerStatus {
    pub queued_tasks: usize,
    pub running_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub total_bytes_processed: u64,
    pub average_speed_mbps: f64,
}