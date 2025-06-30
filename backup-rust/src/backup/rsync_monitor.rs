use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn, error};
use std::time::{Duration, Instant};
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RsyncMetrics {
    pub connection_status: ConnectionStatus,
    pub transfer_stats: TransferStats,
    pub performance_metrics: PerformanceMetrics,
    pub verification_results: Option<VerificationResults>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub is_connected: bool,
    pub target_available: bool,
    pub target_path: PathBuf,
    pub rsync_version: String,
    pub protocol_version: u32,
    pub latency_ms: Option<u64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub file_count: u64,
    pub files_transferred: u64,
    pub files_updated: u64,
    pub files_skipped: u64,
    pub speedup_ratio: f64,
    pub compression_ratio: f64,
    pub current_file: Option<String>,
    pub current_progress: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub average_speed_mbps: f64,
    pub current_speed_mbps: f64,
    pub peak_speed_mbps: f64,
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: u64,
    pub disk_io_mbps: f64,
    pub network_utilization_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResults {
    pub checksum_method: String,
    pub files_verified: u64,
    pub verification_errors: Vec<VerificationError>,
    pub integrity_status: IntegrityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationError {
    pub file_path: String,
    pub expected_checksum: String,
    pub actual_checksum: String,
    pub error_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrityStatus {
    Verified,
    PartiallyVerified,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Default)]
pub struct SimpleMetrics {
    pub bytes_transferred: u64,
    pub files_transferred: u32,
    pub current_progress: u8,
    pub start_time: Option<Instant>,
}

pub struct RsyncMonitor {
    target_path: PathBuf,
    metrics: RsyncMetrics,
    simple_metrics: SimpleMetrics,
    progress_regex: Regex,
    stats_regex: Regex,
    file_regex: Regex,
}

impl RsyncMonitor {
    pub fn new(target_path: PathBuf) -> Self {
        let target_path_clone = target_path.clone();
        Self {
            target_path,
            metrics: RsyncMetrics {
                connection_status: ConnectionStatus {
                    is_connected: false,
                    target_available: false,
                    target_path: target_path_clone,
                    rsync_version: String::new(),
                    protocol_version: 0,
                    latency_ms: None,
                    error_message: None,
                },
                transfer_stats: TransferStats::default(),
                performance_metrics: PerformanceMetrics::default(),
                verification_results: None,
                last_check: chrono::Utc::now(),
            },
            simple_metrics: SimpleMetrics::default(),
            progress_regex: Regex::new(r"(\d+)%").unwrap(),
            stats_regex: Regex::new(r"Number of .*: ([\d,]+)").unwrap(),
            file_regex: Regex::new(r"(?:>f\+{9}|<f[+.]{9})\s+(.+)").unwrap(),
        }
    }
    
    pub fn start_monitoring(&mut self) {
        self.simple_metrics.start_time = Some(Instant::now());
        info!("Started rsync monitoring for {:?}", self.target_path);
    }
    
    pub fn process_output(&mut self, line: &str) {
        // Update metrics based on rsync output
        if let Some(captures) = self.progress_regex.captures(line) {
            if let Some(progress) = captures.get(1) {
                if let Ok(p) = progress.as_str().parse::<u8>() {
                    self.simple_metrics.current_progress = p;
                    self.metrics.transfer_stats.current_progress = p;
                }
            }
        }
        
        // Parse file count from xfr# pattern
        if line.contains("xfr#") {
            if let Some(pos) = line.find("xfr#") {
                let num_str = &line[pos + 4..]
                    .chars()
                    .take_while(|c| c.is_numeric())
                    .collect::<String>();
                
                if let Ok(count) = num_str.parse::<u32>() {
                    self.simple_metrics.files_transferred = count;
                    self.metrics.transfer_stats.files_transferred = count as u64;
                }
            }
        }
        
        // Parse bytes transferred
        if line.contains("bytes") && (line.contains("sent") || line.contains("transferred")) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if *part == "bytes" && i > 0 {
                    if let Ok(bytes) = parts[i-1].replace(",", "").parse::<u64>() {
                        self.simple_metrics.bytes_transferred = bytes;
                        self.metrics.transfer_stats.bytes_sent = bytes;
                    }
                }
            }
        }
        
        // Parse current file
        if let Some(captures) = self.file_regex.captures(line) {
            if let Some(file) = captures.get(1) {
                self.metrics.transfer_stats.current_file = Some(file.as_str().to_string());
            }
        }
    }
    
    pub fn get_metrics(&self) -> SimpleMetrics {
        self.simple_metrics.clone()
    }

    /// Check rsync connection and availability
    pub async fn check_connection(&mut self) -> Result<ConnectionStatus> {
        info!("Checking rsync connection to {}", self.target_path.display());
        
        // Check if target path exists
        if !self.target_path.exists() {
            self.metrics.connection_status.is_connected = false;
            self.metrics.connection_status.target_available = false;
            self.metrics.connection_status.error_message = Some("Target path does not exist".to_string());
            return Ok(self.metrics.connection_status.clone());
        }

        // Check if it's a mount point
        let is_mount = self.check_mount_point(&self.target_path).await?;
        if !is_mount {
            warn!("Target path {} is not a mount point", self.target_path.display());
            self.metrics.connection_status.error_message = Some("Target is not a mounted drive".to_string());
        }

        // Test rsync connection with dry-run
        let start = Instant::now();
        let output = Command::new("rsync")
            .args(&["--version"])
            .output()
            .await
            .context("Failed to execute rsync")?;

        let basic_latency = start.elapsed().as_millis() as u64;

        if output.status.success() {
            let version_info = String::from_utf8_lossy(&output.stdout);
            if let Some(version) = self.parse_rsync_version(&version_info) {
                self.metrics.connection_status.rsync_version = version;
            }
            // Store basic latency if we don't get a better measurement
            self.metrics.connection_status.latency_ms = Some(basic_latency);
        }

        // Test actual connectivity with a small transfer
        let test_result = self.test_rsync_transfer().await;
        match test_result {
            Ok(latency) => {
                self.metrics.connection_status.is_connected = true;
                self.metrics.connection_status.target_available = true;
                self.metrics.connection_status.latency_ms = Some(latency);
                self.metrics.connection_status.error_message = None;
                info!("Rsync connection successful, latency: {}ms", latency);
            }
            Err(e) => {
                self.metrics.connection_status.is_connected = false;
                self.metrics.connection_status.error_message = Some(e.to_string());
                error!("Rsync connection failed: {}", e);
            }
        }

        self.metrics.last_check = chrono::Utc::now();
        Ok(self.metrics.connection_status.clone())
    }

    /// Monitor active rsync process
    pub async fn monitor_process(&mut self, mut child: tokio::process::Child) -> Result<TransferStats> {
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();
        
        let mut last_update = Instant::now();
        let mut speed_samples = Vec::new();

        loop {
            tokio::select! {
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(line)) => {
                            self.process_output_line(&line, &mut speed_samples);
                            
                            // Update metrics periodically
                            if last_update.elapsed() > Duration::from_millis(500) {
                                self.update_performance_metrics(&speed_samples);
                                last_update = Instant::now();
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
                
                err_line = stderr_reader.next_line() => {
                    if let Ok(Some(line)) = err_line {
                        warn!("Rsync stderr: {}", line);
                        self.process_error_line(&line);
                    }
                }
            }
        }

        // Wait for process completion
        let exit_status = child.wait().await?;
        if !exit_status.success() {
            error!("Rsync process failed with status: {}", exit_status);
        }

        Ok(self.metrics.transfer_stats.clone())
    }

    /// Verify backup integrity
    pub async fn verify_backup(&mut self, source: &Path, destination: &Path) -> Result<VerificationResults> {
        info!("Starting backup verification for {}", source.display());
        
        let mut verification = VerificationResults {
            checksum_method: "xxh64".to_string(), // Modern fast checksum
            files_verified: 0,
            verification_errors: Vec::new(),
            integrity_status: IntegrityStatus::InProgress,
        };

        // Run rsync with checksum verification
        let mut cmd = Command::new("rsync");
        cmd.args(&[
            "-avnc",  // Archive, verbose, dry-run, checksum
            "--checksum-choice=xxh64",
            "--itemize-changes",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ]);

        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse verification results
        for line in stdout.lines() {
            verification.files_verified += 1;
            
            if line.starts_with(">f") && line.contains("c") {
                // File differs in checksum
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    verification.verification_errors.push(VerificationError {
                        file_path: parts[1].to_string(),
                        expected_checksum: "source".to_string(),
                        actual_checksum: "destination".to_string(),
                        error_type: "checksum_mismatch".to_string(),
                    });
                }
            }
        }

        // Determine integrity status
        verification.integrity_status = if verification.verification_errors.is_empty() {
            IntegrityStatus::Verified
        } else if verification.verification_errors.len() < (verification.files_verified as usize / 100) {
            IntegrityStatus::PartiallyVerified
        } else {
            IntegrityStatus::Failed
        };

        info!("Verification complete: {} files checked, {} errors found", 
              verification.files_verified, verification.verification_errors.len());

        self.metrics.verification_results = Some(verification.clone());
        Ok(verification)
    }

    // Helper methods
    
    async fn check_mount_point(&self, path: &Path) -> Result<bool> {
        let output = Command::new("findmnt")
            .args(&["-n", "-o", "TARGET", path.to_str().unwrap()])
            .output()
            .await?;
            
        Ok(output.status.success() && !output.stdout.is_empty())
    }

    fn parse_rsync_version(&self, version_output: &str) -> Option<String> {
        version_output.lines()
            .find(|line| line.starts_with("rsync"))
            .and_then(|line| line.split_whitespace().nth(2))
            .map(|v| v.to_string())
    }

    async fn test_rsync_transfer(&self) -> Result<u64> {
        let start = Instant::now();
        
        let output = Command::new("rsync")
            .args(&[
                "--dry-run",
                "--stats",
                "/dev/null",
                &format!("{}/", self.target_path.display()),
            ])
            .output()
            .await?;

        if output.status.success() {
            Ok(start.elapsed().as_millis() as u64)
        } else {
            Err(anyhow::anyhow!("Rsync test transfer failed"))
        }
    }

    fn process_output_line(&mut self, line: &str, speed_samples: &mut Vec<f64>) {
        // Parse progress updates
        if let Some(captures) = self.progress_regex.captures(line) {
            if let Ok(progress) = captures[2].parse::<u8>() {
                self.metrics.transfer_stats.current_progress = progress;
            }
            
            // Parse speed
            if let Some(speed) = self.parse_speed(&captures[3]) {
                speed_samples.push(speed);
                if speed_samples.len() > 100 {
                    speed_samples.remove(0);
                }
                self.metrics.performance_metrics.current_speed_mbps = speed / 1_048_576.0;
            }
        }

        // Parse file transfers
        if let Some(captures) = self.file_regex.captures(line) {
            self.metrics.transfer_stats.current_file = Some(captures[1].to_string());
            self.metrics.transfer_stats.files_transferred += 1;
        }

        // Parse statistics
        if line.contains("Number of files:") {
            if let Some(count) = self.extract_number(line) {
                self.metrics.transfer_stats.file_count = count;
            }
        } else if line.contains("Number of created files:") {
            if let Some(count) = self.extract_number(line) {
                self.metrics.transfer_stats.files_updated = count;
            }
        } else if line.contains("Total bytes sent:") {
            if let Some(bytes) = self.extract_number(line) {
                self.metrics.transfer_stats.bytes_sent = bytes;
            }
        } else if line.contains("Total bytes received:") {
            if let Some(bytes) = self.extract_number(line) {
                self.metrics.transfer_stats.bytes_received = bytes;
            }
        } else if line.contains("Speedup is") {
            if let Some(ratio) = self.extract_float(line) {
                self.metrics.transfer_stats.speedup_ratio = ratio;
            }
        }
    }

    fn process_error_line(&mut self, line: &str) {
        if line.contains("rsync error") || line.contains("ERROR") {
            error!("Rsync error detected: {}", line);
            // Could update error counters or status here
        }
    }

    fn update_performance_metrics(&mut self, speed_samples: &[f64]) {
        if !speed_samples.is_empty() {
            let avg_speed: f64 = speed_samples.iter().sum::<f64>() / speed_samples.len() as f64;
            self.metrics.performance_metrics.average_speed_mbps = avg_speed / 1_048_576.0;
            
            if let Some(&max_speed) = speed_samples.iter().max_by(|a, b| a.partial_cmp(b).unwrap()) {
                self.metrics.performance_metrics.peak_speed_mbps = max_speed / 1_048_576.0;
            }
        }

        // Update system metrics
        if let Ok(sys_info) = self.get_system_metrics() {
            self.metrics.performance_metrics.cpu_usage_percent = sys_info.0;
            self.metrics.performance_metrics.memory_usage_mb = sys_info.1;
        }
    }

    fn parse_speed(&self, speed_str: &str) -> Option<f64> {
        let cleaned = speed_str.replace(",", "");
        let (num_str, unit) = cleaned.split_at(cleaned.len() - 4);
        
        if let Ok(num) = num_str.parse::<f64>() {
            let multiplier = match unit {
                "B/s" => 1.0,
                "KB/s" => 1024.0,
                "MB/s" => 1_048_576.0,
                "GB/s" => 1_073_741_824.0,
                _ => return None,
            };
            Some(num * multiplier)
        } else {
            None
        }
    }

    fn extract_number(&self, line: &str) -> Option<u64> {
        line.split(':')
            .nth(1)?
            .trim()
            .replace(",", "")
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    }

    fn extract_float(&self, line: &str) -> Option<f64> {
        line.split_whitespace()
            .find_map(|word| word.parse::<f64>().ok())
    }

    fn get_system_metrics(&self) -> Result<(f32, u64)> {
        // Placeholder - would integrate with sysinfo crate
        Ok((15.5, 2048))
    }

    pub fn get_full_metrics(&self) -> &RsyncMetrics {
        &self.metrics
    }
}

impl Default for TransferStats {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            file_count: 0,
            files_transferred: 0,
            files_updated: 0,
            files_skipped: 0,
            speedup_ratio: 1.0,
            compression_ratio: 1.0,
            current_file: None,
            current_progress: 0,
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            average_speed_mbps: 0.0,
            current_speed_mbps: 0.0,
            peak_speed_mbps: 0.0,
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0,
            disk_io_mbps: 0.0,
            network_utilization_percent: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rsync_monitor_creation() {
        let monitor = RsyncMonitor::new(PathBuf::from("/tmp"));
        assert!(!monitor.metrics.connection_status.is_connected);
    }

    #[test]
    fn test_speed_parsing() {
        let monitor = RsyncMonitor::new(PathBuf::from("/tmp"));
        assert_eq!(monitor.parse_speed("1.23MB/s"), Some(1.23 * 1_048_576.0));
        assert_eq!(monitor.parse_speed("456KB/s"), Some(456.0 * 1024.0));
        assert_eq!(monitor.parse_speed("789B/s"), Some(789.0));
    }
}