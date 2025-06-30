#!/usr/bin/env python3
"""
Work coordination system for handing off tasks between processes.
"""

import json
import os
import time
import threading
import multiprocessing
import queue
from typing import Dict, List, Any, Optional
from dataclasses import dataclass, asdict
from enum import Enum


class TaskStatus(Enum):
    PENDING = "pending"
    ASSIGNED = "assigned"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class WorkerStatus(Enum):
    IDLE = "idle"
    BUSY = "busy"
    STOPPED = "stopped"
    ERROR = "error"


@dataclass
class Task:
    """Represents a backup task"""
    id: str
    directory_name: str
    directory_path: str
    directory_size: int
    priority: int = 1
    status: TaskStatus = TaskStatus.PENDING
    assigned_worker: Optional[str] = None
    start_time: Optional[float] = None
    end_time: Optional[float] = None
    progress: int = 0
    error_message: Optional[str] = None
    retry_count: int = 0
    max_retries: int = 3


@dataclass
class Worker:
    """Represents a backup worker process"""
    id: str
    pid: Optional[int] = None
    status: WorkerStatus = WorkerStatus.IDLE
    current_task: Optional[str] = None
    last_heartbeat: Optional[float] = None
    total_tasks_completed: int = 0
    total_bytes_processed: int = 0


class WorkCoordinator:
    """Coordinates work distribution between multiple backup processes"""
    
    def __init__(self, work_file: str = "work_queue.json"):
        self.work_file = work_file
        self.lock = threading.Lock()
        self.tasks: Dict[str, Task] = {}
        self.workers: Dict[str, Worker] = {}
        self.task_queue = queue.PriorityQueue()
        self.heartbeat_timeout = 30  # seconds
        
        self.load_work_state()
    
    def load_work_state(self):
        """Load work state from persistent storage"""
        try:
            if os.path.exists(self.work_file):
                with open(self.work_file, 'r') as f:
                    data = json.load(f)
                
                # Load tasks
                for task_data in data.get('tasks', []):
                    task = Task(**task_data)
                    self.tasks[task.id] = task
                
                # Load workers
                for worker_data in data.get('workers', []):
                    worker = Worker(**worker_data)
                    self.workers[worker.id] = worker
                
                print(f"Loaded {len(self.tasks)} tasks and {len(self.workers)} workers")
        
        except Exception as e:
            print(f"Error loading work state: {e}")
    
    def save_work_state(self):
        """Save work state to persistent storage"""
        try:
            with self.lock:
                data = {
                    'tasks': [asdict(task) for task in self.tasks.values()],
                    'workers': [asdict(worker) for worker in self.workers.values()],
                    'timestamp': time.time()
                }
            
            with open(self.work_file, 'w') as f:
                json.dump(data, f, indent=2, default=str)
        
        except Exception as e:
            print(f"Error saving work state: {e}")
    
    def register_worker(self, worker_id: str, pid: int = None) -> bool:
        """Register a new worker process"""
        with self.lock:
            if worker_id in self.workers:
                # Update existing worker
                self.workers[worker_id].pid = pid
                self.workers[worker_id].status = WorkerStatus.IDLE
                self.workers[worker_id].last_heartbeat = time.time()
            else:
                # Create new worker
                self.workers[worker_id] = Worker(
                    id=worker_id,
                    pid=pid,
                    status=WorkerStatus.IDLE,
                    last_heartbeat=time.time()
                )
            
            self.save_work_state()
            print(f"Registered worker: {worker_id} (PID: {pid})")
            return True
    
    def unregister_worker(self, worker_id: str):
        """Unregister a worker process"""
        with self.lock:
            if worker_id in self.workers:
                worker = self.workers[worker_id]
                
                # If worker has a current task, reassign it
                if worker.current_task:
                    task = self.tasks.get(worker.current_task)
                    if task and task.status == TaskStatus.IN_PROGRESS:
                        task.status = TaskStatus.PENDING
                        task.assigned_worker = None
                        print(f"Reassigning task {task.id} due to worker {worker_id} shutdown")
                
                # Remove worker
                del self.workers[worker_id]
                self.save_work_state()
                print(f"Unregistered worker: {worker_id}")
    
    def add_task(self, directory_name: str, directory_path: str, 
                 directory_size: int, priority: int = 1) -> str:
        """Add a new backup task"""
        task_id = f"task_{int(time.time())}_{directory_name}"
        
        with self.lock:
            task = Task(
                id=task_id,
                directory_name=directory_name,
                directory_path=directory_path,
                directory_size=directory_size,
                priority=priority
            )
            
            self.tasks[task_id] = task
            # Add to priority queue (lower priority number = higher priority)
            self.task_queue.put((priority, task_id))
            
            self.save_work_state()
            print(f"Added task: {task_id} for directory {directory_name}")
            return task_id
    
    def get_next_task(self, worker_id: str) -> Optional[Task]:
        """Get the next task for a worker"""
        with self.lock:
            # Check if worker is registered
            if worker_id not in self.workers:
                print(f"Worker {worker_id} not registered")
                return None
            
            # Try to get a task from the queue
            try:
                while not self.task_queue.empty():
                    priority, task_id = self.task_queue.get_nowait()
                    
                    task = self.tasks.get(task_id)
                    if task and task.status == TaskStatus.PENDING:
                        # Assign task to worker
                        task.status = TaskStatus.ASSIGNED
                        task.assigned_worker = worker_id
                        
                        # Update worker
                        worker = self.workers[worker_id]
                        worker.status = WorkerStatus.BUSY
                        worker.current_task = task_id
                        worker.last_heartbeat = time.time()
                        
                        self.save_work_state()
                        print(f"Assigned task {task_id} to worker {worker_id}")
                        return task
            
            except queue.Empty:
                pass
            
            return None
    
    def start_task(self, task_id: str, worker_id: str) -> bool:
        """Mark a task as started"""
        with self.lock:
            task = self.tasks.get(task_id)
            if task and task.assigned_worker == worker_id:
                task.status = TaskStatus.IN_PROGRESS
                task.start_time = time.time()
                
                self.save_work_state()
                print(f"Task {task_id} started by worker {worker_id}")
                return True
            
            return False
    
    def update_task_progress(self, task_id: str, worker_id: str, progress: int):
        """Update task progress"""
        with self.lock:
            task = self.tasks.get(task_id)
            if task and task.assigned_worker == worker_id:
                task.progress = progress
                
                # Update worker heartbeat
                if worker_id in self.workers:
                    self.workers[worker_id].last_heartbeat = time.time()
                
                # Save periodically (every 10% progress)
                if progress % 10 == 0:
                    self.save_work_state()
    
    def complete_task(self, task_id: str, worker_id: str, success: bool = True, 
                     error_message: str = None):
        """Mark a task as completed or failed"""
        with self.lock:
            task = self.tasks.get(task_id)
            if task and task.assigned_worker == worker_id:
                task.end_time = time.time()
                
                if success:
                    task.status = TaskStatus.COMPLETED
                    task.progress = 100
                else:
                    task.status = TaskStatus.FAILED
                    task.error_message = error_message
                    
                    # Retry logic
                    if task.retry_count < task.max_retries:
                        task.retry_count += 1
                        task.status = TaskStatus.PENDING
                        task.assigned_worker = None
                        # Re-add to queue with lower priority
                        self.task_queue.put((task.priority + task.retry_count, task_id))
                        print(f"Retrying task {task_id} (attempt {task.retry_count})")
                
                # Update worker
                if worker_id in self.workers:
                    worker = self.workers[worker_id]
                    worker.status = WorkerStatus.IDLE
                    worker.current_task = None
                    worker.last_heartbeat = time.time()
                    
                    if success:
                        worker.total_tasks_completed += 1
                        worker.total_bytes_processed += task.directory_size
                
                self.save_work_state()
                status = "completed" if success else "failed"
                print(f"Task {task_id} {status} by worker {worker_id}")
    
    def heartbeat(self, worker_id: str, current_task_progress: int = None):
        """Worker heartbeat to indicate it's still alive"""
        with self.lock:
            if worker_id in self.workers:
                worker = self.workers[worker_id]
                worker.last_heartbeat = time.time()
                
                # Update current task progress if provided
                if worker.current_task and current_task_progress is not None:
                    task = self.tasks.get(worker.current_task)
                    if task:
                        task.progress = current_task_progress
    
    def check_worker_health(self):
        """Check if workers are still alive and reassign tasks if needed"""
        current_time = time.time()
        
        with self.lock:
            dead_workers = []
            
            for worker_id, worker in self.workers.items():
                if (worker.last_heartbeat and 
                    current_time - worker.last_heartbeat > self.heartbeat_timeout):
                    
                    print(f"Worker {worker_id} appears dead (no heartbeat for {current_time - worker.last_heartbeat:.1f}s)")
                    dead_workers.append(worker_id)
            
            # Handle dead workers
            for worker_id in dead_workers:
                worker = self.workers[worker_id]
                
                # Reassign current task if any
                if worker.current_task:
                    task = self.tasks.get(worker.current_task)
                    if task and task.status == TaskStatus.IN_PROGRESS:
                        task.status = TaskStatus.PENDING
                        task.assigned_worker = None
                        # Re-add to queue
                        self.task_queue.put((task.priority, task.id))
                        print(f"Reassigned task {task.id} from dead worker {worker_id}")
                
                # Mark worker as stopped
                worker.status = WorkerStatus.STOPPED
                worker.current_task = None
    
    def get_status(self) -> Dict[str, Any]:
        """Get overall system status"""
        with self.lock:
            pending_tasks = [t for t in self.tasks.values() if t.status == TaskStatus.PENDING]
            in_progress_tasks = [t for t in self.tasks.values() if t.status == TaskStatus.IN_PROGRESS]
            completed_tasks = [t for t in self.tasks.values() if t.status == TaskStatus.COMPLETED]
            failed_tasks = [t for t in self.tasks.values() if t.status == TaskStatus.FAILED]
            
            active_workers = [w for w in self.workers.values() if w.status != WorkerStatus.STOPPED]
            
            return {
                'tasks': {
                    'pending': len(pending_tasks),
                    'in_progress': len(in_progress_tasks),
                    'completed': len(completed_tasks),
                    'failed': len(failed_tasks),
                    'total': len(self.tasks)
                },
                'workers': {
                    'active': len(active_workers),
                    'total': len(self.workers)
                },
                'queue_size': self.task_queue.qsize()
            }
    
    def cancel_task(self, task_id: str) -> bool:
        """Cancel a pending task"""
        with self.lock:
            task = self.tasks.get(task_id)
            if task and task.status in [TaskStatus.PENDING, TaskStatus.ASSIGNED]:
                task.status = TaskStatus.CANCELLED
                
                # If assigned to a worker, free the worker
                if task.assigned_worker:
                    worker = self.workers.get(task.assigned_worker)
                    if worker:
                        worker.status = WorkerStatus.IDLE
                        worker.current_task = None
                
                self.save_work_state()
                return True
            
            return False


# Example usage and testing
if __name__ == "__main__":
    coordinator = WorkCoordinator()
    
    # Simulate registering workers
    coordinator.register_worker("worker1", 12345)
    coordinator.register_worker("worker2", 12346)
    
    # Add some tasks
    coordinator.add_task("Documents", "/home/user/Documents", 1000000, priority=1)
    coordinator.add_task("Downloads", "/home/user/Downloads", 2000000, priority=2)
    coordinator.add_task("Projects", "/home/user/Projects", 500000, priority=1)
    
    # Simulate worker getting tasks
    task1 = coordinator.get_next_task("worker1")
    if task1:
        print(f"Worker1 got task: {task1.directory_name}")
        coordinator.start_task(task1.id, "worker1")
        
        # Simulate progress updates
        for progress in [25, 50, 75, 100]:
            coordinator.update_task_progress(task1.id, "worker1", progress)
            time.sleep(0.1)
        
        coordinator.complete_task(task1.id, "worker1", success=True)
    
    # Print status
    print(f"System status: {coordinator.get_status()}")