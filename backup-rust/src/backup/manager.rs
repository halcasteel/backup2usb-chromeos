use super::{BackupSession, BackupState, Directory, SharedSession, DirectoryStatus};
use super::task_manager::TaskManager;
use super::task_processor::{BackupTaskProcessor, TaskProcessor};
use crate::storage::Storage;
use crate::utils::config::Config;
use crate::utils::disk::verify_backup_mount;
use crate::utils::log_buffer::LogBuffer;
use anyhow::Result;
use std::sync::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::broadcast;
use tracing::{debug, info, error, warn};

#[derive(Clone)]
pub struct BackupManager {
    config: Arc<Config>,
    storage: Storage,
    session: SharedSession,
    command_tx: mpsc::Sender<Command>,
    event_tx: broadcast::Sender<Event>,
    task_manager: TaskManager,
    log_buffer: LogBuffer,
}

#[derive(Debug)]
pub enum Command {
    Start { parallel: bool },
    Pause,
    Stop,
    UpdateProgress { index: usize, progress: u8 },
}

#[derive(Debug, Clone)]
pub enum Event {
    StateChanged(BackupState),
    ProgressUpdate { index: usize, progress: u8 },
    DirectoryCompleted { index: usize },
    Error { message: String },
}

impl BackupManager {
    pub fn new(config: Config, storage: Storage) -> Self {
        let (command_tx, command_rx) = mpsc::channel(32);
        let (event_tx, _) = broadcast::channel(128);
        
        // Initialize or load session
        let session = Arc::new(RwLock::new(BackupSession {
            id: uuid::Uuid::new_v4().to_string(),
            directories: Vec::new(),
            current_index: 0,
            total_size: 0,
            completed_size: 0,
            start_time: None,
            state: BackupState::Stopped,
            errors: Vec::new(),
        }));

        // Initialize task manager
        let num_workers = std::cmp::min(
            config.max_workers,
            num_cpus::get() / 2, // Use half the cores for efficiency
        );
        let task_manager = TaskManager::new(num_workers);
        
        let log_buffer = LogBuffer::new(1000); // Keep last 1000 log entries
        
        let manager = Self {
            config: Arc::new(config),
            storage: storage.clone(),
            session: session.clone(),
            command_tx,
            event_tx: event_tx.clone(),
            task_manager,
            log_buffer: log_buffer.clone(),
        };

        // Remove the event listener to prevent deadlock
        // Logs will be added directly in the worker

        // Spawn the command processor with minimal resource usage
        tokio::spawn(Self::process_commands(
            command_rx,
            session,
            storage,
            event_tx,
            manager.config.clone(),
            manager.task_manager.clone(),
            log_buffer,
        ));

        manager
    }

    async fn process_commands(
        mut rx: mpsc::Receiver<Command>,
        session: SharedSession,
        storage: Storage,
        event_tx: broadcast::Sender<Event>,
        config: Arc<Config>,
        mut task_manager: TaskManager,
        log_buffer: LogBuffer,
    ) {
        // Set up the task processor with event channel and log buffer
        let task_processor = Arc::new(BackupTaskProcessor::with_log_buffer(event_tx.clone(), log_buffer.clone())) as Arc<dyn TaskProcessor>;
        task_manager.set_task_processor(task_processor);
        
        let _event_tx_clone = event_tx.clone();

        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Start { parallel } => {
                    info!("Starting backup (parallel: {})", parallel);
                    log_buffer.add_log("info", format!("Starting backup (parallel: {})", parallel), None);
                    log_buffer.add_log("info", "Backup process initiated".to_string(), None);
                    log_buffer.add_log("info", "Scanning directories...".to_string(), None);
                    
                    // Verify backup mount before starting
                    let backup_dest = config.backup_dest.parent()
                        .unwrap_or(&config.backup_dest)
                        .to_string_lossy();
                    
                    match verify_backup_mount(&backup_dest).await {
                        Ok(true) => {
                            info!("Backup mount verified at: {}", backup_dest);
                            
                            // Create the backup destination directory if it doesn't exist
                            if let Err(e) = tokio::fs::create_dir_all(&config.backup_dest).await {
                                error!("Failed to create backup directory: {}", e);
                                let _ = event_tx.send(Event::Error {
                                    message: format!("Failed to create backup directory: {}", e),
                                });
                                continue;
                            }
                            info!("Created backup directory: {}", config.backup_dest.display());
                        }
                        Ok(false) => {
                            error!("Backup destination is not mounted: {}", backup_dest);
                            let _ = event_tx.send(Event::Error {
                                message: format!("USB drive is not mounted at {}. Please mount the drive and try again.", backup_dest),
                            });
                            continue;
                        }
                        Err(e) => {
                            error!("Failed to verify backup mount: {}", e);
                            let _ = event_tx.send(Event::Error {
                                message: format!("Failed to verify backup mount: {}", e),
                            });
                            continue;
                        }
                    }
                    
                    {
                        let mut session = session.write().unwrap();
                        session.state = BackupState::Running;
                        session.start_time = Some(chrono::Utc::now().timestamp());
                    }
                    
                    let _ = event_tx.send(Event::StateChanged(BackupState::Running));
                    
                    // Start the task manager with appropriate number of workers
                    let num_workers = if parallel {
                        std::cmp::min(
                            config.max_workers,
                            num_cpus::get() / 2, // Use half the cores for efficiency
                        )
                    } else {
                        1 // Single worker for sequential
                    };
                    
                    // Start the task manager
                    if let Err(e) = task_manager.start(num_workers, session.clone(), config.clone()).await {
                        error!("Failed to start task manager: {}", e);
                        let _ = event_tx.send(Event::Error {
                            message: format!("Failed to start task manager: {}", e),
                        });
                        continue;
                    }
                    
                    // Create a background task to process directories and add them to task manager
                    let session_clone = session.clone();
                    let task_manager_clone = task_manager.clone();
                    let event_tx_task = event_tx.clone();
                    let log_buffer_clone = log_buffer.clone();
                    
                    tokio::spawn(async move {
                        // Get all selected directories and add them as tasks
                        let directories_to_process: Vec<(usize, u8, u64)> = {
                            let session = session_clone.read().unwrap();
                            session.directories.iter().enumerate()
                                .filter(|(_, dir)| dir.selected && dir.status == DirectoryStatus::Pending)
                                .map(|(idx, dir)| {
                                    // Calculate priority based on size (smaller directories get higher priority)
                                    let priority = if dir.size < 1_000_000 { // < 1MB
                                        100
                                    } else if dir.size < 100_000_000 { // < 100MB
                                        80
                                    } else if dir.size < 1_000_000_000 { // < 1GB
                                        60
                                    } else {
                                        40
                                    };
                                    (idx, priority, dir.size)
                                })
                                .collect()
                        };
                        
                        // Add all directories as tasks
                        let total_directories = directories_to_process.len();
                        log_buffer_clone.add_log("info", format!("Found {} directories to backup", total_directories), None);
                        
                        for (idx, priority, size) in directories_to_process {
                            // Get directory name for logging
                            let dir_name = {
                                let session = session_clone.read().unwrap();
                                session.directories.get(idx).map(|d| d.name.clone()).unwrap_or_default()
                            };
                            let dir_name_for_log = dir_name.clone();
                            log_buffer_clone.add_log("info", format!("Queuing {} for backup", dir_name), Some(dir_name_for_log));
                            task_manager_clone.add_task(idx, priority, size);
                        }
                        
                        // Monitor task manager status and send progress updates
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            
                            // Check if backup is still running
                            {
                                let session = session_clone.read().unwrap();
                                if session.state != BackupState::Running {
                                    break;
                                }
                            }
                            
                            // Get task manager status
                            let status = task_manager_clone.get_status();
                            
                            // Check directory statuses and send progress events
                            {
                                let session = session_clone.read().unwrap();
                                for (idx, dir) in session.directories.iter().enumerate() {
                                    if dir.status == DirectoryStatus::Active && dir.progress > 0 {
                                        let _ = event_tx_task.send(Event::ProgressUpdate {
                                            index: idx,
                                            progress: dir.progress,
                                        });
                                    }
                                }
                            }
                            
                            if status.completed_tasks + status.failed_tasks >= total_directories {
                                info!("All backup tasks completed");
                                // Update session state
                                {
                                    let mut session = session_clone.write().unwrap();
                                    session.state = BackupState::Stopped;
                                }
                                let _ = event_tx_task.send(Event::StateChanged(BackupState::Stopped));
                                break;
                            }
                        }
                    });
                }
                
                Command::Pause => {
                    info!("Pausing backup");
                    session.write().unwrap().state = BackupState::Paused;
                    let _ = event_tx.send(Event::StateChanged(BackupState::Paused));
                }
                
                Command::Stop => {
                    info!("Stopping backup");
                    session.write().unwrap().state = BackupState::Stopped;
                    let _ = event_tx.send(Event::StateChanged(BackupState::Stopped));
                    
                    // Shutdown task manager
                    if let Err(e) = task_manager.shutdown().await {
                        error!("Error shutting down task manager: {}", e);
                    }
                }
                
                Command::UpdateProgress { index, progress } => {
                    debug!("Updating progress: dir {} = {}%", index, progress);
                    {
                        let mut session = session.write().unwrap();
                        if let Some(dir) = session.directories.get_mut(index) {
                            dir.progress = progress;
                            dir.size_copied = (dir.size as f64 * progress as f64 / 100.0) as u64;
                        }
                    }
                    let _ = event_tx.send(Event::ProgressUpdate { index, progress });
                }
            }
            
            // Save state to storage (throttled)
            {
                let session_data = session.read().unwrap().clone();
                let _ = storage.save_session(&session_data).await;
            }
        }
    }

    pub async fn scan_directories(&self) -> Result<()> {
        info!("Scanning directories...");
        
        let scanner = super::scanner::DirectoryScanner::new(self.config.clone());
        let directories = scanner.scan_home_directory().await?;
        
        let num_directories = {
            let mut session = self.session.write().unwrap();
            session.directories = directories;
            session.total_size = session.directories.iter().map(|d| d.size).sum();
            session.directories.len()
        };
        
        let session_data = self.session.read().unwrap().clone();
        self.storage.save_session(&session_data).await?;
        info!("Found {} directories", num_directories);
        
        Ok(())
    }

    pub async fn start(&self, parallel: bool) -> Result<()> {
        self.command_tx.send(Command::Start { parallel }).await?;
        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        self.command_tx.send(Command::Pause).await?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.command_tx.send(Command::Stop).await?;
        Ok(())
    }

    pub async fn get_status(&self) -> BackupStatus {
        let session = self.session.read().unwrap();
        
        BackupStatus {
            state: session.state,
            directories: session.directories.clone(),
            current_index: session.current_index,
            total_size: session.total_size,
            completed_size: session.completed_size,
            start_time: session.start_time,
            errors: session.errors.clone(),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    pub async fn restore_session(&self, session: BackupSession) -> Result<()> {
        info!("Restoring backup session: {}", session.id);
        
        // Update the current session with the restored data
        let num_directories = {
            let mut current_session = self.session.write().unwrap();
            *current_session = session;
            current_session.directories.len()
        };
        
        // Save the restored session to ensure it's persisted
        let session_data = self.session.read().unwrap().clone();
        self.storage.save_session(&session_data).await?;
        
        info!("Session restored successfully with {} directories", num_directories);
        
        Ok(())
    }
    
    pub fn get_logs(&self, limit: Option<usize>) -> Vec<crate::utils::log_buffer::LogEntry> {
        self.log_buffer.get_logs(limit)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupStatus {
    pub state: BackupState,
    pub directories: Vec<Directory>,
    pub current_index: usize,
    pub total_size: u64,
    pub completed_size: u64,
    pub start_time: Option<i64>,
    pub errors: Vec<super::BackupError>,
}