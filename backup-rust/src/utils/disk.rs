use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub source: DiskStats,
    pub backup: DiskStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStats {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub percentage: f32,
    pub available: bool,
}

pub async fn get_disk_usage() -> DiskInfo {
    let home_stats = get_path_stats("/home").await.unwrap_or_default();
    let backup_stats = get_path_stats("/mnt/chromeos/removable/PNYRP60PSSD").await.unwrap_or_default();
    
    DiskInfo {
        source: home_stats,
        backup: backup_stats,
    }
}

pub async fn verify_backup_mount(path: &str) -> Result<bool> {
    let path_obj = Path::new(path);
    
    // Check if path exists
    if !path_obj.exists() {
        return Ok(false);
    }
    
    // For ChromeOS removable media, the device appears under /mnt/chromeos/removable/
    // but isn't a traditional mount point. We verify it's actually a USB device by:
    // 1. Checking the path exists
    // 2. Verifying we can write to it (USB drives are writable)
    if path.contains("/mnt/chromeos/removable/") {
        // Extract the device name (e.g., PNYRP60PSSD) from the path
        if let Some(device_name) = path.split("/mnt/chromeos/removable/")
            .nth(1)
            .and_then(|s| s.split('/').next()) 
        {
            let device_path = format!("/mnt/chromeos/removable/{}", device_name);
            let test_path = Path::new(&device_path);
            
            // Check if the device directory exists and is accessible
            if test_path.exists() && test_path.is_dir() {
                // Try to check if we can access it (this will fail if not mounted)
                match tokio::fs::read_dir(&device_path).await {
                    Ok(_) => return Ok(true),
                    Err(_) => return Ok(false),
                }
            }
        }
        return Ok(false);
    }
    
    // For other systems, check if it's a mount point using mountpoint command
    let output = Command::new("mountpoint")
        .arg("-q")
        .arg(path)
        .output()
        .await?;
    
    Ok(output.status.success())
}

async fn get_path_stats(path: &str) -> Result<DiskStats> {
    let path_obj = Path::new(path);
    
    if !path_obj.exists() {
        return Ok(DiskStats {
            total: 0,
            used: 0,
            free: 0,
            percentage: 0.0,
            available: false,
        });
    }
    
    // Use df command to get disk stats
    let output = Command::new("df")
        .args(&["-B1", path]) // Output in bytes
        .output()
        .await?;
    
    if !output.status.success() {
        return Ok(DiskStats {
            total: 0,
            used: 0,
            free: 0,
            percentage: 0.0,
            available: false,
        });
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = output_str.lines().collect();
    
    if lines.len() < 2 {
        return Ok(DiskStats::default());
    }
    
    // Parse the second line which contains the actual data
    let parts: Vec<&str> = lines[1].split_whitespace().collect();
    if parts.len() < 4 {
        return Ok(DiskStats::default());
    }
    
    let total = parts[1].parse::<u64>().unwrap_or(0);
    let used = parts[2].parse::<u64>().unwrap_or(0);
    let free = parts[3].parse::<u64>().unwrap_or(0);
    let percentage = if total > 0 {
        (used as f32 / total as f32) * 100.0
    } else {
        0.0
    };
    
    Ok(DiskStats {
        total,
        used,
        free,
        percentage,
        available: true,
    })
}

impl Default for DiskStats {
    fn default() -> Self {
        Self {
            total: 0,
            used: 0,
            free: 0,
            percentage: 0.0,
            available: false,
        }
    }
}