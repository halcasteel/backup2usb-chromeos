use anyhow::Result;
use sysinfo::System;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{debug, info, warn};

/// Dynamic resource monitor that adjusts worker count based on system resources
pub struct ResourceMonitor {
    system: Arc<RwLock<System>>,
    config: ResourceConfig,
}

#[derive(Debug, Clone)]
pub struct ResourceConfig {
    /// Minimum number of workers (default: 1)
    pub min_workers: usize,
    
    /// Maximum number of workers (default: CPU cores)
    pub max_workers: usize,
    
    /// Target CPU usage percentage (default: 75%)
    pub target_cpu_usage: f32,
    
    /// Minimum free memory in MB to spawn new worker (default: 512)
    pub min_free_memory_mb: u64,
    
    /// Memory per worker in MB (default: 256)
    pub memory_per_worker_mb: u64,
    
    /// Load average threshold (default: 0.8 per core)
    pub load_avg_per_core: f64,
    
    /// Enable dynamic scaling (default: true)
    pub dynamic_scaling: bool,
    
    /// Scale check interval in seconds (default: 5)
    pub scale_interval_secs: u64,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        let cpu_count = num_cpus::get();
        
        Self {
            min_workers: 1,
            max_workers: cpu_count,
            target_cpu_usage: 75.0,
            min_free_memory_mb: 512,
            memory_per_worker_mb: 256,
            load_avg_per_core: 0.8,
            dynamic_scaling: true,
            scale_interval_secs: 5,
        }
    }
}

impl ResourceMonitor {
    pub fn new(config: ResourceConfig) -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        info!("Resource Monitor initialized:");
        info!("  CPU cores: {} (physical: {})", 
            system.cpus().len(), 
            num_cpus::get_physical()
        );
        info!("  Total memory: {} GB", 
            system.total_memory() / 1_073_741_824
        );
        info!("  Available memory: {} GB", 
            system.available_memory() / 1_073_741_824
        );
        
        Self {
            system: Arc::new(RwLock::new(system)),
            config,
        }
    }

    /// Calculate optimal number of workers based on current system resources
    pub fn calculate_optimal_workers(&self) -> usize {
        if !self.config.dynamic_scaling {
            return self.config.max_workers;
        }

        let mut system = self.system.write().unwrap();
        system.refresh_all();
        
        // Get current metrics
        let cpu_usage = system.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / system.cpus().len() as f32;
        let available_memory_mb = system.available_memory() / 1_048_576;
        let load_avg = System::load_average();
        let cpu_count = system.cpus().len();
        
        debug!("Current system state:");
        debug!("  CPU usage: {:.1}%", cpu_usage);
        debug!("  Available memory: {} MB", available_memory_mb);
        debug!("  Load average: {:.2}, {:.2}, {:.2}", 
            load_avg.one, load_avg.five, load_avg.fifteen
        );
        
        // Start with CPU-based calculation
        let mut optimal_workers = if cpu_usage < self.config.target_cpu_usage {
            // We have CPU headroom
            let headroom = self.config.target_cpu_usage - cpu_usage;
            let additional_workers = (headroom / 10.0) as usize; // Each worker ~10% CPU
            
            self.config.min_workers + additional_workers
        } else {
            // CPU is busy, use fewer workers
            self.config.min_workers
        };
        
        // Adjust based on memory
        let memory_limited_workers = (available_memory_mb / self.config.memory_per_worker_mb) as usize;
        optimal_workers = optimal_workers.min(memory_limited_workers);
        
        // Adjust based on load average
        let load_per_core = load_avg.one / cpu_count as f64;
        if load_per_core > self.config.load_avg_per_core {
            // System is under load, reduce workers
            optimal_workers = optimal_workers.saturating_sub(1);
        }
        
        // Apply bounds
        optimal_workers = optimal_workers
            .max(self.config.min_workers)
            .min(self.config.max_workers);
        
        debug!("Calculated optimal workers: {}", optimal_workers);
        
        optimal_workers
    }

    /// Get current resource utilization
    pub fn get_utilization(&self) -> ResourceUtilization {
        let system = self.system.read().unwrap();
        
        let cpu_usage = system.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / system.cpus().len() as f32;
            
        ResourceUtilization {
            cpu_usage,
            memory_used_mb: (system.total_memory() - system.available_memory()) / 1_048_576,
            memory_total_mb: system.total_memory() / 1_048_576,
            load_average: System::load_average().one,
            optimal_workers: self.calculate_optimal_workers(),
        }
    }

    /// Check if we should scale up (add more workers)
    pub fn should_scale_up(&self, current_workers: usize) -> bool {
        if !self.config.dynamic_scaling {
            return false;
        }

        let optimal = self.calculate_optimal_workers();
        let utilization = self.get_utilization();
        
        // Scale up if:
        // 1. We're below optimal count
        // 2. CPU usage is low
        // 3. We have enough memory
        current_workers < optimal &&
        utilization.cpu_usage < self.config.target_cpu_usage - 10.0 &&
        (utilization.memory_total_mb - utilization.memory_used_mb) > 
            (self.config.min_free_memory_mb + self.config.memory_per_worker_mb)
    }

    /// Check if we should scale down (remove workers)
    pub fn should_scale_down(&self, current_workers: usize) -> bool {
        if !self.config.dynamic_scaling {
            return false;
        }

        let optimal = self.calculate_optimal_workers();
        let utilization = self.get_utilization();
        
        // Scale down if:
        // 1. We're above optimal count
        // 2. CPU usage is high
        // 3. Memory is low
        // 4. Load is high
        current_workers > optimal ||
        utilization.cpu_usage > self.config.target_cpu_usage + 10.0 ||
        (utilization.memory_total_mb - utilization.memory_used_mb) < self.config.min_free_memory_mb ||
        utilization.load_average > (self.config.load_avg_per_core * num_cpus::get() as f64)
    }

    /// Start monitoring loop
    pub async fn start_monitoring(
        self: Arc<Self>,
        worker_control: Arc<dyn WorkerControl + Send + Sync>,
    ) {
        let interval = tokio::time::Duration::from_secs(self.config.scale_interval_secs);
        let mut ticker = tokio::time::interval(interval);
        
        loop {
            ticker.tick().await;
            
            let current_workers = worker_control.get_worker_count().await;
            
            if self.should_scale_up(current_workers) {
                let optimal = self.calculate_optimal_workers();
                let to_add = optimal.saturating_sub(current_workers);
                
                if to_add > 0 {
                    info!("Scaling up: adding {} workers (current: {}, optimal: {})",
                        to_add, current_workers, optimal);
                    
                    if let Err(e) = worker_control.add_workers(to_add).await {
                        warn!("Failed to add workers: {}", e);
                    }
                }
            } else if self.should_scale_down(current_workers) {
                let optimal = self.calculate_optimal_workers();
                let to_remove = current_workers.saturating_sub(optimal);
                
                if to_remove > 0 && current_workers > self.config.min_workers {
                    info!("Scaling down: removing {} workers (current: {}, optimal: {})",
                        to_remove, current_workers, optimal);
                    
                    if let Err(e) = worker_control.remove_workers(to_remove).await {
                        warn!("Failed to remove workers: {}", e);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceUtilization {
    pub cpu_usage: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub load_average: f64,
    pub optimal_workers: usize,
}

/// Trait for controlling workers dynamically
#[async_trait::async_trait]
pub trait WorkerControl {
    async fn get_worker_count(&self) -> usize;
    async fn add_workers(&self, count: usize) -> Result<()>;
    async fn remove_workers(&self, count: usize) -> Result<()>;
}

/// Smart resource allocation based on backup characteristics
pub struct SmartAllocator {
    monitor: Arc<ResourceMonitor>,
}

impl SmartAllocator {
    pub fn new(monitor: Arc<ResourceMonitor>) -> Self {
        Self { monitor }
    }
    
    /// Allocate workers based on backup workload characteristics
    pub fn allocate_for_workload(&self, workload: &WorkloadProfile) -> WorkerAllocation {
        let base_optimal = self.monitor.calculate_optimal_workers();
        
        // Adjust based on workload type
        let adjusted = match workload.backup_type {
            BackupType::FullBackup => {
                // Full backups need more workers
                (base_optimal as f32 * 1.2) as usize
            }
            BackupType::Incremental => {
                // Incremental needs fewer workers
                (base_optimal as f32 * 0.8) as usize
            }
            BackupType::LargeFiles => {
                // Large files benefit from fewer but dedicated workers
                base_optimal.min(4)
            }
            BackupType::SmallFiles => {
                // Many small files benefit from more workers
                base_optimal.max(4)
            }
        };
        
        let final_count = adjusted
            .max(self.monitor.config.min_workers)
            .min(self.monitor.config.max_workers);
        
        WorkerAllocation {
            worker_count: final_count,
            memory_per_worker_mb: if workload.total_size > 100_000_000_000 { // 100GB
                512 // More memory for large backups
            } else {
                256
            },
            priority_hint: match workload.backup_type {
                BackupType::FullBackup => "high",
                BackupType::Incremental => "normal",
                BackupType::LargeFiles => "io-bound",
                BackupType::SmallFiles => "cpu-bound",
            },
        }
    }
}

#[derive(Debug)]
pub struct WorkloadProfile {
    pub backup_type: BackupType,
    pub total_size: u64,
    pub file_count: u64,
    pub directory_count: usize,
}

#[derive(Debug)]
pub enum BackupType {
    FullBackup,
    Incremental,
    LargeFiles,  // Few large files
    SmallFiles,  // Many small files
}

#[derive(Debug)]
pub struct WorkerAllocation {
    pub worker_count: usize,
    pub memory_per_worker_mb: u64,
    pub priority_hint: &'static str,
}