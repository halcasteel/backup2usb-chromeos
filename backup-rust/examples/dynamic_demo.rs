/// Example demonstrating dynamic worker scaling based on system resources
use backup_system::utils::{ResourceMonitor, ResourceConfig};
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::main]
async fn main() {
    // Initialize logging
    backup_system::utils::logging::init_tracing();
    
    println!("=== Dynamic Worker Scaling Demo ===\n");
    
    // Create resource monitor with custom config
    let config = ResourceConfig {
        min_workers: 1,
        max_workers: 16, // Allow up to 16 workers
        target_cpu_usage: 75.0,
        min_free_memory_mb: 512,
        memory_per_worker_mb: 256,
        load_avg_per_core: 0.8,
        dynamic_scaling: true,
        scale_interval_secs: 2, // Check every 2 seconds for demo
    };
    
    let monitor = Arc::new(ResourceMonitor::new(config));
    
    // Show system info
    let utilization = monitor.get_utilization();
    println!("System Information:");
    println!("  CPU cores: {}", num_cpus::get());
    println!("  Physical cores: {}", num_cpus::get_physical());
    println!("  Total memory: {} GB", utilization.memory_total_mb / 1024);
    println!();
    
    // Simulate different load scenarios
    println!("Simulating different system loads...\n");
    
    for i in 0..5 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let util = monitor.get_utilization();
        let optimal = monitor.calculate_optimal_workers();
        
        println!("Iteration {}:", i + 1);
        println!("  Current CPU usage: {:.1}%", util.cpu_usage);
        println!("  Memory used: {} MB / {} MB", util.memory_used_mb, util.memory_total_mb);
        println!("  Load average: {:.2}", util.load_average);
        println!("  → Optimal workers: {}", optimal);
        
        // Simulate decision making
        let current_workers = 4; // Assume we have 4 workers
        if monitor.should_scale_up(current_workers) {
            println!("  → Decision: SCALE UP (add workers)");
        } else if monitor.should_scale_down(current_workers) {
            println!("  → Decision: SCALE DOWN (remove workers)");
        } else {
            println!("  → Decision: MAINTAIN current workers");
        }
        println!();
    }
    
    // Demonstrate workload-based allocation
    use backup_system::backup::dynamic_task_manager::DynamicTaskManager;
    use backup_system::backup::{Directory, DirectoryStatus};
    
    println!("\n=== Workload-Based Allocation ===\n");
    
    // Create sample directories
    let directories = vec![
        Directory {
            name: "Documents".to_string(),
            path: "/home/user/Documents".into(),
            size: 5_000_000_000, // 5GB
            status: DirectoryStatus::Pending,
            progress: 0,
            selected: true,
            start_time: None,
            end_time: None,
            files_processed: 0,
            size_copied: 0,
            file_count: Some(50000),
            average_speed: None,
        },
        Directory {
            name: "Videos".to_string(),
            path: "/home/user/Videos".into(),
            size: 50_000_000_000, // 50GB
            status: DirectoryStatus::Pending,
            progress: 0,
            selected: true,
            start_time: None,
            end_time: None,
            files_processed: 0,
            size_copied: 0,
            file_count: Some(100),
            average_speed: None,
        },
    ];
    
    let workload = DynamicTaskManager::analyze_workload(&directories);
    println!("Workload Analysis:");
    println!("  Type: {:?}", workload.backup_type);
    println!("  Total size: {} GB", workload.total_size / 1_073_741_824);
    println!("  Directory count: {}", workload.directory_count);
    println!("  Estimated files: {}", workload.file_count);
    
    let allocator = backup_system::utils::resource_monitor::SmartAllocator::new(monitor.clone());
    let allocation = allocator.allocate_for_workload(&workload);
    
    println!("\nSmart Allocation:");
    println!("  Recommended workers: {}", allocation.worker_count);
    println!("  Memory per worker: {} MB", allocation.memory_per_worker_mb);
    println!("  Priority hint: {}", allocation.priority_hint);
    
    println!("\n✓ Dynamic scaling ensures optimal resource usage!");
    println!("✓ System adapts to workload characteristics!");
    println!("✓ Prevents resource exhaustion automatically!");
}