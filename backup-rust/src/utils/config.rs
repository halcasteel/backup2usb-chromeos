use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server port
    pub port: u16,
    
    /// Database URL
    pub database_url: String,
    
    /// Backup destination path
    pub backup_dest: PathBuf,
    
    /// Home directory to scan
    pub home_dir: String,
    
    /// Maximum number of workers (0 = auto based on CPU)
    pub max_workers: usize,
    
    /// Rsync exclude patterns
    pub rsync_excludes: Vec<String>,
    
    /// Enable dynamic worker scaling
    pub dynamic_scaling: bool,
    
    /// Target CPU usage percentage for scaling
    pub target_cpu_usage: f32,
    
    /// Memory per worker in MB
    pub memory_per_worker: u64,
}

impl Default for Config {
    fn default() -> Self {
        // Use a single backup directory that rsync will keep updated incrementally
        let backup_dest = PathBuf::from("/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup");
        
        Self {
            port: 8888,
            database_url: "sqlite://backup_system.db".to_string(),
            backup_dest,
            home_dir: dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/home".to_string()),
            max_workers: 0, // Auto-detect
            rsync_excludes: vec![
                "venv".to_string(),
                ".venv".to_string(),
                "node_modules".to_string(),
                "__pycache__".to_string(),
                "*.pyc".to_string(),
                ".git/objects".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".cache".to_string(),
                "*.log".to_string(),
                "*.tmp".to_string(),
                "*.swp".to_string(),
            ],
            dynamic_scaling: true,
            target_cpu_usage: 75.0,
            memory_per_worker: 256,
        }
    }
}

impl Config {
    /// Load config from environment and files
    pub fn load() -> Result<Self> {
        let mut config = Config::default();
        
        // Override with environment variables
        if let Ok(port) = std::env::var("BACKUP_PORT") {
            config.port = port.parse()?;
        }
        
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            config.database_url = db_url;
        }
        
        if let Ok(dest) = std::env::var("BACKUP_DEST") {
            config.backup_dest = PathBuf::from(dest);
        }
        
        if let Ok(workers) = std::env::var("MAX_WORKERS") {
            config.max_workers = workers.parse()?;
        }
        
        if let Ok(scaling) = std::env::var("DYNAMIC_SCALING") {
            config.dynamic_scaling = scaling.parse()?;
        }
        
        // Auto-detect workers if not set
        if config.max_workers == 0 {
            config.max_workers = num_cpus::get();
        }
        
        Ok(config)
    }
}

pub fn load_config() -> Result<Config> {
    Config::load()
}