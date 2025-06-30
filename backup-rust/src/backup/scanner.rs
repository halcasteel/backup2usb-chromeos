use super::{Directory, DirectoryStatus};
use crate::utils::config::Config;
use anyhow::Result;
use std::path::Path;
use tokio::fs;
use std::sync::Arc;
use tracing::{info, debug};

pub struct DirectoryScanner {
    config: Arc<Config>,
}

impl DirectoryScanner {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub async fn scan_home_directory(&self) -> Result<Vec<Directory>> {
        let home_path = Path::new(&self.config.home_dir);
        if !home_path.exists() {
            return Err(anyhow::anyhow!("Home directory does not exist: {}", self.config.home_dir));
        }

        info!("Scanning directories in {}", self.config.home_dir);
        let mut directories = Vec::new();

        // Read home directory
        let mut entries = fs::read_dir(home_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Skip if not a directory
            if !path.is_dir() {
                continue;
            }

            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };

            // Skip hidden directories
            if name.starts_with('.') {
                debug!("Skipping hidden directory: {}", name);
                continue;
            }

            // Skip excluded directories
            if self.config.rsync_excludes.iter().any(|exc| name == *exc) {
                debug!("Skipping excluded directory: {}", name);
                continue;
            }

            // For faster scanning, just get metadata size initially
            // Full size calculation can happen later
            let size = match fs::metadata(&path).await {
                Ok(metadata) => metadata.len(),
                Err(_) => 1024, // Default size if we can't read metadata
            };

            info!("Found directory: {}", name);
            
            directories.push(Directory {
                name: name.clone(),
                path: path.clone(),
                size,
                status: DirectoryStatus::Pending,
                progress: 0,
                selected: true, // Select all by default
                start_time: None,
                end_time: None,
                files_processed: 0,
                size_copied: 0,
                file_count: None,
                average_speed: None,
                current_file: None,
                bytes_processed: None,
            });
        }

        // Sort directories by name (descending) to match original behavior
        directories.sort_by(|a, b| b.name.cmp(&a.name));

        Ok(directories)
    }

}