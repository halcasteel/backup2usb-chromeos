pub mod manager;
pub mod worker;
pub mod rsync;
pub mod scanner;
pub mod task_manager;
pub mod dynamic_task_manager;
pub mod rsync_monitor;
pub mod task_processor;

pub use manager::BackupManager;
pub use worker::BackupWorker;
pub use task_manager::TaskManager;
pub use dynamic_task_manager::DynamicTaskManager;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directory {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub status: DirectoryStatus,
    pub progress: u8,
    pub selected: bool,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub files_processed: u64,
    pub size_copied: u64,
    pub file_count: Option<u64>,
    pub average_speed: Option<u64>,
    pub current_file: Option<String>,
    pub bytes_processed: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DirectoryStatus {
    Pending,
    Active,
    Completed,
    Error,
    Skipped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupState {
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSession {
    pub id: String,
    pub directories: Vec<Directory>,
    pub current_index: usize,
    pub total_size: u64,
    pub completed_size: u64,
    pub start_time: Option<i64>,
    pub state: BackupState,
    pub errors: Vec<BackupError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupError {
    pub directory: String,
    pub message: String,
    pub timestamp: i64,
}

// Shared state for zero-copy access  
pub type SharedDirectory = Arc<std::sync::RwLock<Directory>>;
pub type SharedSession = Arc<std::sync::RwLock<BackupSession>>;