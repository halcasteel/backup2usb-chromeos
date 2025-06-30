use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::backup::{BackupManager, BackupState};

mod compat;

pub fn routes() -> Router<BackupManager> {
    Router::new()
        .route("/status", get(get_status))
        .route("/control", post(control_backup))
        .route("/logs", post(get_logs))
        .route("/logs/download", get(download_logs))
        .route("/profile", post(set_profile))
        .route("/select", post(select_directories))
        .route("/dryrun", post(set_dryrun))
        .route("/schedule", post(save_schedule))
        // Add compatibility routes for frontend
        .merge(compat::compat_routes())
}

#[derive(Serialize)]
struct StatusResponse {
    // Match frontend expectations
    state: String,
    directories: Vec<DirectoryInfo>,
    #[serde(rename = "totalSize")]
    total_size: u64,
    #[serde(rename = "completedSize")]
    completed_size: u64,
    #[serde(rename = "startTime")]
    start_time: Option<i64>,
    #[serde(rename = "lastCompletedDir")]
    last_completed_dir: Option<String>,
    #[serde(rename = "nextDir")]
    next_dir: Option<String>,
    logs: Vec<LogEntry>,
    history: Vec<HistoryEntry>,
    #[serde(rename = "localDiskSpace")]
    local_disk_space: Option<DiskSpace>,
    #[serde(rename = "remoteDiskSpace")]
    remote_disk_space: Option<DiskSpace>,
    #[serde(rename = "mountStatus")]
    mount_status: MountStatus,
    #[serde(rename = "currentOperation")]
    current_operation: Option<CurrentOperation>,
    #[serde(rename = "speedHistory")]
    speed_history: Vec<SpeedEntry>,
}

#[derive(Serialize)]
struct DiskSpace {
    free: u64,
    total: u64,
    used: u64,
    percentage: f32,
}

#[derive(Serialize)]
struct MountStatus {
    mounted: bool,
    path: String,
    message: Option<String>,
}

#[derive(Serialize)]
struct CurrentOperation {
    name: String,
    progress: u8,
    files_processed: u64,
    size_copied: String,
    time_elapsed: String,
    current_speed: String,
}

#[derive(Serialize)]
struct DirectoryInfo {
    name: String,
    path: String,
    size: u64,
    status: String,
    progress: u8,
    selected: bool,
    #[serde(rename = "filesProcessed")]
    files_processed: Option<u64>,
    #[serde(rename = "bytesProcessed")]
    bytes_processed: Option<u64>,
    #[serde(rename = "currentFile")]
    current_file: Option<String>,
    #[serde(rename = "averageSpeed")]
    average_speed: Option<u64>,
    #[serde(rename = "fileCount")]
    file_count: Option<u64>,
}

#[derive(Serialize)]
struct Profile {
    name: String,
    directories: Vec<String>,
}

#[derive(Serialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
    directory: Option<String>,
}

#[derive(Serialize)]
struct SpeedEntry {
    timestamp: i64,
    speed: f32,
}

#[derive(Serialize)]
struct HistoryEntry {
    id: String,
    timestamp: String,
    duration: u64,
    size: u64,
    #[serde(rename = "filesCount")]
    files_count: u64,
    status: String,
}

pub async fn get_status(State(manager): State<BackupManager>) -> Json<StatusResponse> {
    // Scan directories if empty (but don't block)
    if manager.get_status().await.directories.is_empty() {
        let _ = manager.scan_directories().await;
    }
    
    let status = manager.get_status().await;
    
    // Get config to check backup destination
    let config = crate::utils::config::load_config().unwrap_or_default();
    let backup_dest = config.backup_dest.parent()
        .unwrap_or(&config.backup_dest)
        .to_string_lossy()
        .to_string();
    
    // Check if backup drive is mounted
    let mount_status = match crate::utils::disk::verify_backup_mount(&backup_dest).await {
        Ok(true) => MountStatus {
            mounted: true,
            path: backup_dest.clone(),
            message: None,
        },
        Ok(false) => MountStatus {
            mounted: false,
            path: backup_dest.clone(),
            message: Some(format!("USB drive is not mounted at {}. Please mount the drive before starting backup.", backup_dest)),
        },
        Err(e) => MountStatus {
            mounted: false,
            path: backup_dest.clone(),
            message: Some(format!("Failed to verify mount status: {}", e)),
        },
    };
    
    // Convert state enum to string
    let state_str = match status.state {
        BackupState::Running => "running",
        BackupState::Paused => "paused",
        BackupState::Stopped => "idle",
    }.to_string();
    
    // Find last completed and next directory
    let last_completed = status.directories.iter()
        .filter(|d| d.status == crate::backup::DirectoryStatus::Completed)
        .last()
        .map(|d| d.name.clone());
    
    let next_dir = status.directories.iter()
        .find(|d| d.status == crate::backup::DirectoryStatus::Pending && d.selected)
        .map(|d| d.name.clone());
    
    // Convert directories to frontend format
    let directories = status.directories.iter()
        .map(|d| DirectoryInfo {
            name: d.name.clone(),
            path: d.path.to_string_lossy().to_string(),
            size: d.size,
            status: format!("{:?}", d.status).to_lowercase(),
            progress: d.progress,
            selected: d.selected,
            files_processed: Some(d.files_processed),
            bytes_processed: d.bytes_processed,
            current_file: d.current_file.clone(),
            average_speed: d.average_speed,
            file_count: d.file_count,
        })
        .collect();
    
    // Get disk space info
    let disk_info = crate::utils::disk::get_disk_usage().await;
    let local_disk = Some(DiskSpace {
        free: disk_info.source.free,
        total: disk_info.source.total,
        used: disk_info.source.used,
        percentage: disk_info.source.percentage,
    });
    
    let remote_disk = if disk_info.backup.available {
        Some(DiskSpace {
            free: disk_info.backup.free,
            total: disk_info.backup.total,
            used: disk_info.backup.used,
            percentage: disk_info.backup.percentage,
        })
    } else {
        None
    };
    
    // Calculate current operation if backup is running
    let current_operation = if matches!(status.state, BackupState::Running) {
        if let Some(current_dir) = status.directories.iter()
            .find(|d| d.status == crate::backup::DirectoryStatus::Active) {
            
            let elapsed = status.start_time
                .map(|start| chrono::Utc::now().timestamp() - start)
                .unwrap_or(0);
            
            let speed = calculate_current_speed(&status);
            
            Some(CurrentOperation {
                name: current_dir.name.clone(),
                progress: current_dir.progress,
                files_processed: current_dir.files_processed,
                size_copied: format_bytes(current_dir.bytes_processed.unwrap_or(0)),
                time_elapsed: format_duration(elapsed as u64),
                current_speed: format!("{:.1} MB/s", speed),
            })
        } else {
            None
        }
    } else {
        None
    };
    
    // Create speed history entries
    let speed_history = if matches!(status.state, BackupState::Running) {
        vec![SpeedEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            speed: calculate_current_speed(&status) as f32,
        }]
    } else {
        Vec::new()
    };
    
    Json(StatusResponse {
        state: state_str,
        directories,
        total_size: status.total_size,
        completed_size: status.completed_size,
        start_time: status.start_time,
        last_completed_dir: last_completed,
        next_dir: next_dir,
        logs: {
            // Get recent logs
            let logs = manager.get_logs(Some(50)); // Get 50 most recent logs
            logs.into_iter().map(|log| {
                let level = match log.level.as_str() {
                    "error" => "error",
                    "warn" | "warning" => "warning", 
                    "success" | "info" | _ => "info",
                }.to_string();
                
                LogEntry {
                    timestamp: chrono::DateTime::from_timestamp(log.timestamp, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| log.timestamp.to_string()),
                    level,
                    message: log.message,
                    directory: log.directory,
                }
            }).collect()
        },
        history: Vec::new(), // TODO: Load from storage
        local_disk_space: local_disk,
        remote_disk_space: remote_disk,
        mount_status,
        current_operation,
        speed_history,
    })
}

#[derive(Deserialize)]
struct ControlRequest {
    action: String,
    parallel: Option<bool>,
}

async fn control_backup(
    State(manager): State<BackupManager>,
    Json(req): Json<ControlRequest>,
) -> Json<serde_json::Value> {
    // For start action, verify mount first
    if req.action == "start" {
        let config = crate::utils::config::load_config().unwrap_or_default();
        let backup_dest = config.backup_dest.parent()
            .unwrap_or(&config.backup_dest)
            .to_string_lossy();
        
        match crate::utils::disk::verify_backup_mount(&backup_dest).await {
            Ok(true) => {
                // Mount is verified, proceed
            }
            Ok(false) => {
                return Json(serde_json::json!({
                    "error": format!("USB drive is not mounted at {}. Please mount the drive and try again.", backup_dest)
                }));
            }
            Err(e) => {
                return Json(serde_json::json!({
                    "error": format!("Failed to verify backup mount: {}", e)
                }));
            }
        }
    }
    
    let result = match req.action.as_str() {
        "start" => manager.start(req.parallel.unwrap_or(true)).await,
        "pause" => manager.pause().await,
        "stop" => manager.stop().await,
        _ => return Json(serde_json::json!({"error": "Invalid action"})),
    };
    
    match result {
        Ok(_) => Json(serde_json::json!({"status": "ok"})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

// Helper functions
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1_048_576 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    }
}

fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

fn calculate_current_speed(status: &crate::backup::manager::BackupStatus) -> f64 {
    // First try to get speed from active directory
    if let Some(active_dir) = status.directories.iter()
        .find(|d| d.status == crate::backup::DirectoryStatus::Active) {
        if let Some(speed) = active_dir.average_speed {
            return speed as f64 / 1_048_576.0; // Convert to MB/s
        }
    }
    
    // Fallback: Calculate based on completed size and elapsed time
    if let Some(start_time) = status.start_time {
        let elapsed = chrono::Utc::now().timestamp() - start_time;
        if elapsed > 0 && status.completed_size > 0 {
            return (status.completed_size as f64 / elapsed as f64) / 1_048_576.0; // MB/s
        }
    }
    0.0
}

fn calculate_eta(status: &crate::backup::manager::BackupStatus, speed_mbps: f64) -> String {
    if speed_mbps > 0.0 {
        let remaining = status.total_size - status.completed_size;
        let eta_seconds = (remaining as f64 / (speed_mbps * 1_048_576.0)) as u64;
        format_duration(eta_seconds)
    } else {
        "calculating...".to_string()
    }
}

// Simplified logs endpoint
async fn get_logs(State(manager): State<BackupManager>) -> Json<serde_json::Value> {
    let logs = manager.get_logs(Some(100)); // Get 100 recent logs
    let formatted_logs: Vec<_> = logs.into_iter().map(|log| {
        serde_json::json!({
            "timestamp": chrono::DateTime::from_timestamp(log.timestamp, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| log.timestamp.to_string()),
            "level": log.level,
            "message": log.message,
            "directory": log.directory,
        })
    }).collect();
    
    Json(serde_json::json!({"logs": formatted_logs}))
}

async fn download_logs() -> String {
    "Log content here".to_string()
}

async fn set_profile(Json(_data): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn select_directories(Json(_data): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn set_dryrun(Json(_data): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn save_schedule(Json(_data): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}