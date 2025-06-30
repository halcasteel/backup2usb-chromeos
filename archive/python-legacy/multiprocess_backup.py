#!/usr/bin/env python3
"""
Multi-process backup implementation for improved performance.

This module provides an alternative backup manager that can run multiple
rsync processes in parallel while coordinating their progress.
"""

import json
import os
import time
import subprocess
import threading
import concurrent.futures
import queue
from typing import List, Dict, Any
import logging

logger = logging.getLogger(__name__)

class MultiProcessBackupManager:
    """Backup manager that can run multiple directories in parallel"""
    
    def __init__(self, max_workers=3):
        self.max_workers = max_workers
        self.executor = None
        self.progress_queue = queue.Queue()
        self.active_processes = {}
        self.lock = threading.Lock()
        
    def can_run_parallel(self, directories: List[Dict]) -> bool:
        """
        Determine if directories can be backed up in parallel.
        Consider I/O constraints, destination space, etc.
        """
        # Check if we have multiple directories
        if len(directories) <= 1:
            return False
            
        # Check total size vs available space
        total_size = sum(d.get('size', 0) for d in directories)
        
        # For now, enable parallel if we have more than 2 dirs and reasonable sizes
        return len(directories) >= 2
    
    def calculate_optimal_workers(self, directories: List[Dict]) -> int:
        """Calculate optimal number of worker processes based on system resources"""
        import psutil
        
        # Consider CPU cores
        cpu_cores = psutil.cpu_count(logical=False) or 2
        
        # Consider available memory
        memory_gb = psutil.virtual_memory().available / (1024**3)
        
        # Consider I/O characteristics
        # For USB drives, too many parallel processes can hurt performance
        # Start conservative with 2-3 processes max
        optimal = min(
            self.max_workers,
            cpu_cores // 2,  # Use half the physical cores
            len(directories),  # Don't exceed number of directories
            3  # Cap at 3 for USB drive performance
        )
        
        logger.info(f"Calculated optimal workers: {optimal} (CPU cores: {cpu_cores}, Memory: {memory_gb:.1f}GB)")
        return max(1, optimal)
    
    def backup_directory_worker(self, dir_info: Dict, backup_dest: str, 
                               progress_callback=None) -> Dict:
        """Worker function to backup a single directory"""
        worker_id = threading.current_thread().name
        logger.info(f"[{worker_id}] Starting backup of {dir_info['name']}")
        
        try:
            dir_info['status'] = 'active'
            dir_info['startTime'] = time.time()
            dir_info['workerId'] = worker_id
            
            if progress_callback:
                progress_callback(dir_info)
            
            # Build rsync command
            dest_path = os.path.join(backup_dest, dir_info["name"])
            cmd = [
                'rsync', '-avzP',
                '--no-perms', '--no-owner', '--no-group',
                '--exclude=venv', '--exclude=.venv', '--exclude=env', '--exclude=.env',
                '--exclude=node_modules', '--exclude=__pycache__', '--exclude=*.pyc',
                '--exclude=.git/objects', '--exclude=dist', '--exclude=build',
                '--exclude=.next', '--exclude=.cache', '--exclude=*.log',
                '--exclude=*.tmp', '--exclude=*.swp',
                '--info=progress2',
                '--stats',
                dir_info["path"] + '/',
                dest_path + '/'
            ]
            
            # Track the process
            with self.lock:
                process = subprocess.Popen(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    universal_newlines=True,
                    bufsize=1
                )
                self.active_processes[worker_id] = process
            
            # Monitor progress
            for line in process.stdout:
                if '%' in line:
                    try:
                        parts = line.strip().split()
                        if len(parts) > 1 and '%' in parts[1]:
                            progress = int(parts[1].rstrip('%'))
                            dir_info["progress"] = progress
                            dir_info["sizeCopied"] = int(dir_info["size"] * progress / 100)
                            
                            if progress_callback:
                                progress_callback(dir_info)
                    except:
                        pass
            
            process.wait()
            
            # Clean up
            with self.lock:
                if worker_id in self.active_processes:
                    del self.active_processes[worker_id]
            
            # Update final status
            if process.returncode == 0:
                dir_info['status'] = 'completed'
                dir_info['progress'] = 100
                dir_info['sizeCopied'] = dir_info['size']
                dir_info['endTime'] = time.time()
                dir_info['duration'] = dir_info['endTime'] - dir_info['startTime']
                dir_info['averageSpeed'] = dir_info['size'] / dir_info['duration'] if dir_info['duration'] > 0 else 0
                logger.info(f"[{worker_id}] Completed backup of {dir_info['name']} in {dir_info['duration']:.1f}s")
            else:
                dir_info['status'] = 'error'
                dir_info['endTime'] = time.time()
                dir_info['duration'] = dir_info['endTime'] - dir_info['startTime']
                logger.error(f"[{worker_id}] Failed backup of {dir_info['name']}: return code {process.returncode}")
            
            if progress_callback:
                progress_callback(dir_info)
                
            return dir_info
            
        except Exception as e:
            logger.error(f"[{worker_id}] Exception backing up {dir_info['name']}: {e}", exc_info=True)
            dir_info['status'] = 'error'
            dir_info['error'] = str(e)
            if progress_callback:
                progress_callback(dir_info)
            return dir_info
    
    def start_parallel_backup(self, directories: List[Dict], backup_dest: str, 
                            progress_callback=None) -> bool:
        """Start parallel backup of multiple directories"""
        if not self.can_run_parallel(directories):
            logger.info("Parallel backup not suitable, falling back to sequential")
            return False
        
        selected_dirs = [d for d in directories if d.get('selected', True) and d['status'] == 'pending']
        
        if not selected_dirs:
            logger.info("No directories selected for backup")
            return False
        
        optimal_workers = self.calculate_optimal_workers(selected_dirs)
        logger.info(f"Starting parallel backup with {optimal_workers} workers for {len(selected_dirs)} directories")
        
        # Create thread pool
        self.executor = concurrent.futures.ThreadPoolExecutor(
            max_workers=optimal_workers,
            thread_name_prefix="BackupWorker"
        )
        
        # Submit backup tasks
        futures = []
        for dir_info in selected_dirs:
            future = self.executor.submit(
                self.backup_directory_worker,
                dir_info,
                backup_dest,
                progress_callback
            )
            futures.append(future)
        
        # Wait for completion
        try:
            for future in concurrent.futures.as_completed(futures):
                result = future.result()
                logger.info(f"Directory backup completed: {result['name']} -> {result['status']}")
        except Exception as e:
            logger.error(f"Error in parallel backup: {e}", exc_info=True)
        finally:
            self.executor.shutdown(wait=True)
            self.executor = None
        
        return True
    
    def stop_all_backups(self):
        """Stop all running backup processes"""
        with self.lock:
            for worker_id, process in self.active_processes.items():
                logger.info(f"Terminating backup process {worker_id}")
                process.terminate()
            self.active_processes.clear()
        
        if self.executor:
            self.executor.shutdown(wait=False)
            self.executor = None
    
    def get_status(self) -> Dict:
        """Get status of all running workers"""
        with self.lock:
            return {
                'active_workers': len(self.active_processes),
                'worker_ids': list(self.active_processes.keys()),
                'max_workers': self.max_workers
            }


# Integration point - this could be used by the main backup_server.py
def should_use_parallel_backup(directories: List[Dict]) -> bool:
    """Determine if parallel backup would be beneficial"""
    manager = MultiProcessBackupManager()
    return manager.can_run_parallel(directories)


if __name__ == "__main__":
    # Test the multiprocess backup functionality
    print("MultiProcess Backup Manager - Test Mode")
    manager = MultiProcessBackupManager(max_workers=2)
    
    # Mock directories for testing
    test_dirs = [
        {"name": "test1", "size": 1000000, "path": "/tmp/test1", "selected": True, "status": "pending"},
        {"name": "test2", "size": 2000000, "path": "/tmp/test2", "selected": True, "status": "pending"}
    ]
    
    print(f"Can run parallel: {manager.can_run_parallel(test_dirs)}")
    print(f"Optimal workers: {manager.calculate_optimal_workers(test_dirs)}")