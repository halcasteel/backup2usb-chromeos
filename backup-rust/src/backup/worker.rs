use super::{BackupState, DirectoryStatus, SharedSession};
use super::rsync_monitor::RsyncMonitor;
use crate::utils::config::Config;
use anyhow::Result;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

pub struct BackupWorker {
    id: usize,
    session: SharedSession,
    event_tx: broadcast::Sender<super::manager::Event>,
    config: Arc<Config>,
    log_buffer: Option<crate::utils::log_buffer::LogBuffer>,
}

impl BackupWorker {
    pub fn new(
        id: usize,
        session: SharedSession,
        event_tx: broadcast::Sender<super::manager::Event>,
        config: Arc<Config>,
        log_buffer: Option<crate::utils::log_buffer::LogBuffer>,
    ) -> Self {
        Self {
            id,
            session,
            event_tx,
            config,
            log_buffer,
        }
    }

    /// Process a single directory without autonomous looping
    /// This method is now called by the TaskManager for each directory
    pub async fn process_single_directory(&self, directory_index: usize) -> Result<()> {
        info!("Worker {} processing directory index {}", self.id, directory_index);
        
        // Check if we should stop
        {
            let session = self.session.read().unwrap();
            if session.state != BackupState::Running {
                return Err(anyhow::anyhow!("Backup session is not running"));
            }
        }
        
        // Process the directory
        match self.process_directory(directory_index).await {
            Ok(()) => {
                info!("Worker {} successfully processed directory {}", self.id, directory_index);
                Ok(())
            }
            Err(e) => {
                error!("Worker {} error processing directory {}: {}", self.id, directory_index, e);
                
                // Mark as error
                {
                    let mut session = self.session.write().unwrap();
                    if let Some(dir) = session.directories.get_mut(directory_index) {
                        dir.status = DirectoryStatus::Error;
                    }
                    let dir_name = session.directories.get(directory_index)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    session.errors.push(super::BackupError {
                        directory: dir_name,
                        message: e.to_string(),
                        timestamp: chrono::Utc::now().timestamp(),
                    });
                }
                
                Err(e)
            }
        }
    }

    pub async fn process_directory(&self, index: usize) -> Result<()> {
        let (name, path, size) = {
            let session = self.session.read().unwrap();
            let dir = &session.directories[index];
            (dir.name.clone(), dir.path.clone(), dir.size)
        };
        
        info!("Worker {}: Processing {} ({} bytes)", self.id, name, size);
        
        // Add log entry for starting backup
        if let Some(log_buffer) = &self.log_buffer {
            log_buffer.add_log(
                "info",
                format!("Starting backup of {} ({:.2} MB)", name, size as f64 / 1_048_576.0),
                Some(name.clone())
            );
        }
        
        // Initialize rsync monitor
        let mut monitor = RsyncMonitor::new(PathBuf::from(&self.config.backup_dest));
        
        // Check connection first
        let connection_status = monitor.check_connection().await?;
        if !connection_status.is_connected {
            return Err(anyhow::anyhow!("Backup destination not available: {:?}", connection_status.error_message));
        }
        
        // Create backup destination directory if it doesn't exist
        let dest = format!("{}/{}", self.config.backup_dest.display(), name);
        tokio::fs::create_dir_all(&dest).await?;
        
        // Build rsync command
        let mut cmd = Command::new("rsync");
        
        cmd.args([
            "-avz",
            "--progress",
            "--no-perms",
            "--no-owner", 
            "--no-group",
            "--info=progress2,stats2,flist2",  // More detailed output
            "--stats",
            "--human-readable",  // Human readable sizes
            "--itemize-changes", // Show what changed
            "--update",  // Only copy files that are newer than destination
            "--delete",  // Remove files from dest that don't exist in source
        ]);
        
        // Add excludes
        for exclude in &self.config.rsync_excludes {
            cmd.arg(format!("--exclude={}", exclude));
        }
        
        cmd.arg(format!("{}/", path.to_string_lossy()));
        cmd.arg(format!("{}/", dest));
        
        debug!("Running rsync command: {:?}", cmd);
        
        // Spawn rsync process
        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        
        // Process output efficiently
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        // Spawn task to capture stderr
        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            let mut errors = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!("rsync stderr: {}", line);
                errors.push(line);
            }
            errors
        });
        
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        let mut last_progress = 0u8;
        let mut files_processed = 0u64;
        let mut current_file = String::new();
        let mut bytes_transferred = 0u64;
        let _total_files = 0u64;
        let mut files_to_transfer = 0u64;
        let mut total_file_count = 0u64;
        let mut files_checked = 0u64;
        let mut last_speed_bytes = 0u64;
        let mut last_speed_time = std::time::Instant::now();
        let mut speed_samples: Vec<f64> = Vec::new();
        let mut initial_scan_complete = false;
        let start_time = std::time::Instant::now();
        
        // Feed output to rsync monitor
        monitor.start_monitoring();
        
        while let Some(line) = lines.next_line().await? {
            debug!("rsync output: {}", line);
            
            // Feed line to monitor
            monitor.process_output(&line);
            
            // Parse rsync output for various metrics
            if line.contains("Number of files:") {
                if let Some(total) = parse_total_files(&line) {
                    total_file_count = total;
                    let mut session = self.session.write().unwrap();
                    if let Some(dir) = session.directories.get_mut(index) {
                        dir.file_count = Some(total);
                    }
                }
            } else if line.contains("Number of created files:") || line.contains("Number of regular files transferred:") {
                if let Some(count) = parse_number_from_line(&line) {
                    files_to_transfer = count;
                }
            } else if line.contains("Total transferred file size:") {
                if let Some(size) = parse_size_from_line(&line) {
                    bytes_transferred = size;
                }
            } else if line.contains("sent ") && line.contains(" bytes") && line.contains("received ") {
                // Parse speed from final stats line: "sent X bytes  received Y bytes  Z bytes/sec"
                if let Some(speed) = parse_speed_from_stats(&line) {
                    let mut session = self.session.write().unwrap();
                    if let Some(dir) = session.directories.get_mut(index) {
                        dir.average_speed = Some(speed as u64);
                    }
                }
            }
            
            // Special handling for initial file scan - rsync lists all files first
            if !initial_scan_complete && total_file_count > 0 && files_checked == 0 {
                // When rsync starts, it scans all files. Show this as initial progress
                let mut session = self.session.write().unwrap();
                if let Some(dir) = session.directories.get_mut(index) {
                    if dir.progress == 0 {
                        dir.progress = 5; // Show 5% to indicate scanning started
                    }
                }
            }
            
            // Parse progress from rsync output
            if let Some(progress) = parse_rsync_progress(&line) {
                if progress != last_progress {
                    last_progress = progress;
                    
                    // Log progress updates every 25%
                    if progress % 25 == 0 && progress > 0 {
                        if let Some(log_buffer) = &self.log_buffer {
                            log_buffer.add_log(
                                "info",
                                format!("{}: {}% complete", name, progress),
                                Some(name.clone())
                            );
                        }
                    }
                    
                    // Get metrics from monitor
                    let metrics = monitor.get_metrics();
                    if metrics.bytes_transferred > 0 {
                        bytes_transferred = metrics.bytes_transferred;
                        files_processed = metrics.files_transferred as u64;
                    }
                    
                    // Calculate instant speed using sliding window
                    let current_time = std::time::Instant::now();
                    let time_delta = current_time.duration_since(last_speed_time).as_secs_f64();
                    let bytes_delta = bytes_transferred.saturating_sub(last_speed_bytes);
                    
                    let instant_speed_mbps = if time_delta > 0.1 && bytes_delta > 0 {
                        let speed = (bytes_delta as f64 / 1_048_576.0) / time_delta;
                        last_speed_bytes = bytes_transferred;
                        last_speed_time = current_time;
                        
                        // Add to speed samples for averaging
                        speed_samples.push(speed);
                        if speed_samples.len() > 10 {
                            speed_samples.remove(0);
                        }
                        
                        speed
                    } else {
                        // Use average of recent speeds
                        if !speed_samples.is_empty() {
                            speed_samples.iter().sum::<f64>() / speed_samples.len() as f64
                        } else {
                            0.0
                        }
                    };
                    
                    // Update session
                    {
                        let mut session = self.session.write().unwrap();
                        if let Some(dir) = session.directories.get_mut(index) {
                            dir.progress = progress;
                            dir.size_copied = bytes_transferred;
                            dir.files_processed = files_processed;
                            dir.bytes_processed = Some(bytes_transferred);
                            dir.average_speed = Some((instant_speed_mbps * 1_048_576.0) as u64);
                            if !current_file.is_empty() {
                                dir.current_file = Some(current_file.clone());
                            }
                        }
                        
                        // Update total completed size
                        // Only count actual bytes transferred, not directory sizes
                        session.completed_size = session.directories.iter()
                            .map(|d| {
                                // For all directories, use bytes_processed or size_copied
                                // This reflects actual backup progress, not just rsync completion
                                d.bytes_processed.unwrap_or(d.size_copied)
                            })
                            .sum();
                    }
                    
                    // Send progress event
                    let _ = self.event_tx.send(super::manager::Event::ProgressUpdate {
                        index,
                        progress,
                    });
                }
            }
            
            // Parse current file being transferred
            if !line.trim().is_empty() && !line.contains("speedup") && !line.contains("total size") {
                // Look for itemized output (starts with > or <)
                if line.starts_with('>') || line.starts_with('<') {
                    if let Some(file) = line.split_whitespace().skip(1).next() {
                        current_file = file.to_string();
                        files_checked += 1;
                        
                        // Only count as processed if actually transferred (>f....... pattern)
                        if line.starts_with(">f") {
                            files_processed += 1;
                        }
                        
                        let mut session = self.session.write().unwrap();
                        if let Some(dir) = session.directories.get_mut(index) {
                            dir.current_file = Some(current_file.clone());
                            dir.files_processed = files_processed;
                        }
                    }
                    initial_scan_complete = true;
                } else if let Some(file) = line.split_whitespace().last() {
                    if file.len() > 1 && !file.starts_with('[') && file.contains('/') {
                        current_file = file.to_string();
                        let mut session = self.session.write().unwrap();
                        if let Some(dir) = session.directories.get_mut(index) {
                            dir.current_file = Some(current_file.clone());
                        }
                    }
                }
            }
            
            // Parse file count from xfr# pattern
            if line.contains("xfr#") {
                if let Some(count) = parse_file_count(&line) {
                    files_processed = count;
                }
            }
            
            // Parse actual transfer data from progress lines
            if line.contains("MB/s") || line.contains("kB/s") || line.contains("GB/s") {
                if let Some(speed_info) = parse_speed_from_progress_line(&line) {
                    let (bytes, speed_bps) = speed_info;
                    if bytes > bytes_transferred {
                        bytes_transferred = bytes;
                        
                        // Update speed samples
                        let speed_mbps = speed_bps / 1_048_576.0;
                        speed_samples.push(speed_mbps);
                        if speed_samples.len() > 10 {
                            speed_samples.remove(0);
                        }
                        
                        let mut session = self.session.write().unwrap();
                        if let Some(dir) = session.directories.get_mut(index) {
                            dir.bytes_processed = Some(bytes_transferred);
                            dir.average_speed = Some((speed_mbps * 1_048_576.0) as u64);
                        }
                    }
                }
            }
        }
        
        // Wait for process to complete
        let status = child.wait().await?;
        let errors = stderr_task.await.unwrap_or_default();
        
        // Get final metrics
        let final_metrics = monitor.get_metrics();
        
        if status.success() {
            // Check if this was a dry run (nothing to transfer)
            let was_dry_run = files_to_transfer == 0 && total_file_count > 0;
            
            // Mark as completed
            {
                let mut session = self.session.write().unwrap();
                if let Some(dir) = session.directories.get_mut(index) {
                    dir.status = DirectoryStatus::Completed;
                    // Only mark as 100% if we actually transferred all the data
                    if bytes_transferred > 0 && dir.size > 0 {
                        dir.progress = ((bytes_transferred as f64 / dir.size as f64) * 100.0).min(100.0) as u8;
                    } else if bytes_transferred == 0 && dir.size > 0 {
                        // No data transferred means this directory was already backed up
                        // Keep existing progress or set to 0 if never backed up
                        dir.progress = if dir.bytes_processed.unwrap_or(0) > 0 { dir.progress } else { 0 };
                    } else {
                        dir.progress = 100; // Empty directory
                    }
                    
                    // Always use actual bytes transferred, not directory size
                    // This ensures progress reflects real backup state
                    dir.size_copied = bytes_transferred;
                    dir.files_processed = files_processed;
                    dir.bytes_processed = Some(bytes_transferred);
                    
                    // For directories that are already up-to-date
                    if was_dry_run || bytes_transferred == 0 {
                        // Files were only checked, not transferred
                        dir.files_processed = 0; // No files actually transferred
                        
                        // Calculate check speed for already-synced directories
                        let elapsed = start_time.elapsed().as_secs_f64();
                        if elapsed > 0.0 {
                            // Show the check speed rather than transfer speed
                            let check_speed = (total_file_count as f64 * 100.0) / elapsed; // Files per second * 100KB average
                            dir.average_speed = Some((check_speed * 1024.0) as u64);
                        }
                    } else {
                        // Files were actually transferred
                        if let Some(start) = dir.start_time {
                            let duration = chrono::Utc::now().timestamp() - start;
                            if duration > 0 && bytes_transferred > 0 {
                                dir.average_speed = Some(bytes_transferred / duration as u64);
                            }
                        }
                    }
                    
                    dir.end_time = Some(chrono::Utc::now().timestamp());
                    dir.file_count = Some(total_file_count);
                }
            }
            
            let _ = self.event_tx.send(super::manager::Event::DirectoryCompleted { index });
            info!("Worker {}: Completed {} - {} files, {} bytes", 
                self.id, name, final_metrics.files_transferred, final_metrics.bytes_transferred);
                
            // Log completion
            if let Some(log_buffer) = &self.log_buffer {
                log_buffer.add_log(
                    "success",
                    format!("Completed backup of {} - {} files, {:.2} MB", 
                        name, 
                        final_metrics.files_transferred,
                        final_metrics.bytes_transferred as f64 / 1_048_576.0
                    ),
                    Some(name.clone())
                );
            }
        } else {
            // Handle rsync errors
            let error_msg = if !errors.is_empty() {
                errors.join("\n")
            } else {
                format!("Rsync failed with exit code: {:?}", status.code())
            };
            
            error!("Worker {}: Failed backing up {}: {}", self.id, name, error_msg);
            
            // Log error
            if let Some(log_buffer) = &self.log_buffer {
                log_buffer.add_log(
                    "error",
                    format!("Failed to backup {}: {}", name, error_msg),
                    Some(name.clone())
                );
            }
            
            return Err(anyhow::anyhow!("Rsync failed: {}", error_msg));
        }
        
        Ok(())
    }
}

fn parse_rsync_progress(line: &str) -> Option<u8> {
    // Parse rsync progress2 format: "          1,234  56%    1.23MB/s    0:00:01"
    if line.contains('%') {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for part in parts {
            if part.ends_with('%') {
                if let Ok(progress) = part.trim_end_matches('%').parse::<u8>() {
                    return Some(progress.min(100));
                }
            }
        }
    }
    None
}

fn parse_file_count(line: &str) -> Option<u64> {
    // Parse xfr#123 pattern
    if let Some(pos) = line.find("xfr#") {
        let num_str = &line[pos + 4..]
            .chars()
            .take_while(|c| c.is_numeric())
            .collect::<String>();
        
        if let Ok(count) = num_str.parse::<u64>() {
            return Some(count);
        }
    }
    None
}

fn parse_total_files(line: &str) -> Option<u64> {
    // Parse "Number of files: 1,234" pattern
    if line.contains("Number of files:") {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 2 {
            let trimmed = parts[1].trim().replace(",", "");
            let num_str = trimmed.split_whitespace().next()?;
            return num_str.parse::<u64>().ok();
        }
    }
    None
}

fn parse_number_from_line(line: &str) -> Option<u64> {
    // Parse numbers from lines like "Number of created files: 123"
    if let Some(colon_pos) = line.find(':') {
        let after_colon = &line[colon_pos + 1..].trim();
        let num_str = after_colon.split_whitespace().next()?.replace(",", "");
        return num_str.parse::<u64>().ok();
    }
    None
}

fn parse_size_from_line(line: &str) -> Option<u64> {
    // Parse size from lines like "Total transferred file size: 1,234,567 bytes"
    if line.contains("Total transferred file size:") {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 2 {
            let size_part = parts[1].trim();
            // Extract number before "bytes"
            if let Some(bytes_pos) = size_part.find(" bytes") {
                let num_str = size_part[..bytes_pos].trim().replace(",", "");
                return num_str.parse::<u64>().ok();
            }
        }
    }
    None
}

fn parse_speed_from_stats(line: &str) -> Option<f64> {
    // Parse speed from: "sent X bytes  received Y bytes  Z bytes/sec"
    if line.contains("bytes/sec") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            if part == &"bytes/sec" && i > 0 {
                if let Ok(bytes_per_sec) = parts[i-1].replace(",", "").parse::<f64>() {
                    return Some(bytes_per_sec);
                }
            }
        }
    }
    None
}

fn parse_speed_from_progress_line(line: &str) -> Option<(u64, f64)> {
    // Parse progress lines like "123,456,789 100%   12.34MB/s    0:01:23"
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    if parts.len() >= 3 {
        // Find the speed part (contains MB/s, kB/s, or GB/s)
        for (i, part) in parts.iter().enumerate() {
            if part.contains("/s") {
                // Parse bytes from the first part
                let bytes_str = parts.get(0)?.replace(",", "");
                let bytes = bytes_str.parse::<u64>().ok()?;
                
                // Parse speed
                let speed_str = part.trim();
                let (num_part, unit) = if speed_str.ends_with("GB/s") {
                    (&speed_str[..speed_str.len() - 4], 1_073_741_824.0)
                } else if speed_str.ends_with("MB/s") {
                    (&speed_str[..speed_str.len() - 4], 1_048_576.0)
                } else if speed_str.ends_with("kB/s") {
                    (&speed_str[..speed_str.len() - 4], 1024.0)
                } else if speed_str.ends_with("B/s") {
                    (&speed_str[..speed_str.len() - 3], 1.0)
                } else {
                    return None;
                };
                
                if let Ok(speed_num) = num_part.parse::<f64>() {
                    return Some((bytes, speed_num * unit));
                }
            }
        }
    }
    None
}