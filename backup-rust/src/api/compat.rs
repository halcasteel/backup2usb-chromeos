use axum::{
    extract::State,
    response::Json,
    routing::post,
    Router,
};
use serde_json::json;

use crate::backup::BackupManager;

// Add compatibility routes that match the original Python API
pub fn compat_routes() -> Router<BackupManager> {
    Router::new()
        .route("/start", post(start_backup))
        .route("/pause", post(pause_backup))
        .route("/stop", post(stop_backup))
}

async fn start_backup(State(manager): State<BackupManager>) -> Json<serde_json::Value> {
    // Verify mount before starting
    let config = crate::utils::config::load_config().unwrap_or_default();
    let backup_dest = config.backup_dest.parent()
        .unwrap_or(&config.backup_dest)
        .to_string_lossy();
    
    match crate::utils::disk::verify_backup_mount(&backup_dest).await {
        Ok(true) => {
            // Mount is verified, proceed
        }
        Ok(false) => {
            return Json(json!({
                "error": format!("USB drive is not mounted at {}. Please mount the drive and try again.", backup_dest)
            }));
        }
        Err(e) => {
            return Json(json!({
                "error": format!("Failed to verify backup mount: {}", e)
            }));
        }
    }
    
    match manager.start(true).await {
        Ok(_) => Json(json!({"status": "started"})),
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}

async fn pause_backup(State(manager): State<BackupManager>) -> Json<serde_json::Value> {
    match manager.pause().await {
        Ok(_) => Json(json!({"status": "paused"})),
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}

async fn stop_backup(State(manager): State<BackupManager>) -> Json<serde_json::Value> {
    match manager.stop().await {
        Ok(_) => Json(json!({"status": "stopped"})),
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}