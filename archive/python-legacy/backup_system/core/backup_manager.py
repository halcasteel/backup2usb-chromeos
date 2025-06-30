#!/usr/bin/env python3
"""
Core backup management system using A2A coordination.
"""

import os
import time
import asyncio
import threading
from typing import Dict, List, Any, Optional
from dataclasses import dataclass
import json

from ..config.settings import get_config
from ..workers.agent_protocol import BackupAgent, AgentCapability, TaskHandoff
from ..workers.work_coordinator import WorkCoordinator


@dataclass
class DirectoryInfo:
    """Information about a directory to backup"""
    name: str
    path: str
    size: int
    status: str = "pending"
    progress: int = 0
    selected: bool = True
    start_time: Optional[float] = None
    end_time: Optional[float] = None
    duration: Optional[float] = None
    files_processed: int = 0
    size_copied: int = 0
    file_count: Optional[int] = None
    average_speed: Optional[float] = None
    assigned_agent: Optional[str] = None


@dataclass 
class BackupSession:
    """Represents a complete backup session"""
    session_id: str
    directories: List[DirectoryInfo]
    start_time: Optional[float] = None
    end_time: Optional[float] = None
    total_size: int = 0
    completed_size: int = 0
    state: str = "stopped"  # stopped, running, paused
    current_dir: Optional[DirectoryInfo] = None
    last_completed_dir: Optional[DirectoryInfo] = None
    next_dir: Optional[DirectoryInfo] = None
    errors: List[Dict[str, Any]] = None
    active_profile: Optional[str] = None
    
    def __post_init__(self):
        if self.errors is None:
            self.errors = []


class ModularBackupManager:
    """
    Modern backup manager using A2A coordination between worker agents.
    Replaces the monolithic BackupManager with a distributed approach.
    """
    
    def __init__(self):
        self.config = get_config()
        self.session: Optional[BackupSession] = None
        self.coordinator_agent: Optional[BackupAgent] = None
        self.worker_agents: List[BackupAgent] = []
        self.work_coordinator = WorkCoordinator()
        
        # State tracking
        self.lock = threading.Lock()
        self.logs: List[Dict[str, Any]] = []
        self.speed_history: List[Dict[str, Any]] = []
        self.profiles: Dict[str, Any] = {}
        self.history: List[Dict[str, Any]] = []
        
        # Load existing state
        self._load_state()
    
    def _load_state(self):
        """Load existing backup state and configuration"""
        try:
            # Load status
            if os.path.exists(self.config.backup_status_file):
                with open(self.config.backup_status_file, 'r') as f:
                    data = json.load(f)
                    self._restore_session_from_data(data)
            
            # Load profiles
            if os.path.exists(self.config.profiles_file):
                with open(self.config.profiles_file, 'r') as f:
                    profile_data = json.load(f)
                    self.profiles = profile_data.get('profiles', {})
            
            # Load history
            if os.path.exists(self.config.backup_history_file):
                with open(self.config.backup_history_file, 'r') as f:
                    history_data = json.load(f)
                    self.history = history_data.get('history', [])
        
        except Exception as e:
            print(f"Error loading state: {e}")
            self._initialize_default_session()
    
    def _restore_session_from_data(self, data: Dict[str, Any]):
        """Restore backup session from saved data"""
        directories = []
        for dir_data in data.get('directories', []):
            # Convert old format to new format
            cleaned_data = {
                'name': dir_data.get('name', ''),
                'path': dir_data.get('path', ''),
                'size': dir_data.get('size', 0),
                'status': dir_data.get('status', 'pending'),
                'progress': dir_data.get('progress', 0),
                'selected': dir_data.get('selected', True),
                'start_time': dir_data.get('startTime'),
                'end_time': dir_data.get('endTime'),
                'duration': dir_data.get('duration'),
                'files_processed': dir_data.get('filesProcessed', 0),
                'size_copied': dir_data.get('sizeCopied', 0),
                'file_count': dir_data.get('fileCount'),
                'average_speed': dir_data.get('averageSpeed'),
                'assigned_agent': dir_data.get('assignedAgent')
            }
            directories.append(DirectoryInfo(**cleaned_data))
        
        self.session = BackupSession(
            session_id=data.get('session_id', f"session_{int(time.time())}"),
            directories=directories,
            start_time=data.get('startTime'),
            end_time=data.get('endTime'),
            total_size=data.get('totalSize', 0),
            completed_size=data.get('completedSize', 0),
            state=data.get('state', 'stopped'),
            active_profile=data.get('activeProfile')
        )
        
        # Restore other state
        self.logs = data.get('logs', [])
        self.speed_history = data.get('speedHistory', [])
    
    def _initialize_default_session(self):
        """Initialize a new default backup session"""
        self.session = BackupSession(
            session_id=f"session_{int(time.time())}",
            directories=[]
        )
        self._discover_directories()
    
    async def initialize_agents(self, num_workers: int = 3):
        """Initialize the A2A coordination system with worker agents"""
        
        # Create coordinator agent
        self.coordinator_agent = BackupAgent(
            "backup-coordinator",
            "Backup Coordinator",
            8800,
            [AgentCapability.COORDINATE, AgentCapability.MONITOR]
        )
        
        # Create worker agents
        base_port = 8801
        for i in range(num_workers):
            capabilities = [AgentCapability.BACKUP_SEQUENTIAL]
            
            # Give different capabilities to different workers
            if i == 0:
                capabilities.append(AgentCapability.BACKUP_PARALLEL)
            if i == 1:
                capabilities.append(AgentCapability.COMPRESS)
            if i == 2:
                capabilities.append(AgentCapability.ENCRYPT)
            
            worker = BackupAgent(
                f"backup-worker-{i+1}",
                f"Backup Worker {i+1}",
                base_port + i,
                capabilities
            )
            self.worker_agents.append(worker)
        
        # Start all agents
        await self.coordinator_agent.start_server()
        for worker in self.worker_agents:
            await worker.start_server()
        
        # Discovery phase - let all agents find each other
        discovery_endpoints = [self.coordinator_agent.endpoint]
        discovery_endpoints.extend([w.endpoint for w in self.worker_agents])
        
        await self.coordinator_agent.discover_peers(discovery_endpoints)
        for worker in self.worker_agents:
            await worker.discover_peers(discovery_endpoints)
        
        print(f"Initialized A2A coordination with {num_workers} workers")
    
    def _discover_directories(self):
        """Discover directories in the home folder"""
        try:
            home = os.path.expanduser("~")
            directories = []
            
            for item in os.listdir(home):
                path = os.path.join(home, item)
                if os.path.isdir(path) and not item.startswith('.'):
                    try:
                        size = self._get_directory_size(path)
                        directories.append(DirectoryInfo(
                            name=item,
                            path=path,
                            size=size,
                            status="pending",
                            selected=True
                        ))
                    except Exception:
                        pass
            
            # Sort by name descending
            directories.sort(key=lambda x: x.name, reverse=True)
            
            # Add important dot directories
            for dot_dir in ['.ssh', '.config', '.gnupg']:
                path = os.path.join(home, dot_dir)
                if os.path.exists(path):
                    size = self._get_directory_size(path)
                    directories.append(DirectoryInfo(
                        name=dot_dir,
                        path=path,
                        size=size,
                        status="pending",
                        selected=True
                    ))
            
            self.session.directories = directories
            self.session.total_size = sum(d.size for d in directories)
            
        except Exception as e:
            print(f"Error discovering directories: {e}")
    
    def _get_directory_size(self, path: str) -> int:
        """Get directory size in bytes"""
        try:
            import subprocess
            result = subprocess.run(['du', '-sb', path], capture_output=True, text=True)
            if result.returncode == 0:
                return int(result.stdout.split()[0])
        except Exception:
            pass
        return 0
    
    async def start_backup(self, use_parallel: bool = True) -> bool:
        """Start the backup process using A2A coordination"""
        if not self.session:
            return False
        
        if self.session.state == "running":
            print("Backup already running")
            return False
        
        # Ensure agents are initialized
        if not self.coordinator_agent:
            await self.initialize_agents()
        
        self.session.state = "running"
        self.session.start_time = time.time()
        
        # Reset directories if starting fresh
        for dir_info in self.session.directories:
            if dir_info.status == "error":
                dir_info.status = "pending"
                dir_info.progress = 0
                dir_info.size_copied = 0
        
        self._save_state()
        
        if use_parallel and len(self.session.directories) > 1:
            await self._start_parallel_backup()
        else:
            await self._start_sequential_backup()
        
        return True
    
    async def _start_parallel_backup(self):
        """Start parallel backup using A2A coordination"""
        selected_dirs = [d for d in self.session.directories if d.selected and d.status == "pending"]
        
        print(f"Starting parallel backup of {len(selected_dirs)} directories")
        
        # Distribute tasks among available workers
        for dir_info in selected_dirs:
            # Find best agent for this task
            best_agent = await self.coordinator_agent.find_best_agent_for_task({
                'capability': AgentCapability.BACKUP_SEQUENTIAL,
                'directory_size': dir_info.size
            })
            
            if best_agent:
                # Hand off task to the best agent
                task_data = {
                    'directory_name': dir_info.name,
                    'directory_path': dir_info.path,
                    'directory_size': dir_info.size,
                    'backup_dest': self.config.backup_dest,
                    'priority': 1,
                    'required_capability': AgentCapability.BACKUP_SEQUENTIAL
                }
                
                success = await self.coordinator_agent.handoff_task(
                    f"backup_{dir_info.name}_{int(time.time())}",
                    best_agent,
                    task_data
                )
                
                if success:
                    dir_info.status = "assigned"
                    dir_info.assigned_agent = best_agent
                    print(f"Assigned {dir_info.name} to {best_agent}")
                else:
                    print(f"Failed to assign {dir_info.name}")
            else:
                print(f"No available agent for {dir_info.name}")
        
        # Monitor progress (simplified for now)
        await self._monitor_parallel_progress()
    
    async def _start_sequential_backup(self):
        """Start sequential backup (fallback mode)"""
        print("Starting sequential backup")
        
        for dir_info in self.session.directories:
            if not dir_info.selected or dir_info.status != "pending":
                continue
            
            if self.session.state != "running":
                break
            
            self.session.current_dir = dir_info
            dir_info.status = "active"
            dir_info.start_time = time.time()
            
            # Find next directory
            next_dir = self._find_next_directory(dir_info)
            self.session.next_dir = next_dir
            
            self._save_state()
            
            # Simulate backup process (replace with actual rsync)
            await self._simulate_backup(dir_info)
            
            # Mark as completed
            dir_info.status = "completed"
            dir_info.end_time = time.time()
            dir_info.duration = dir_info.end_time - dir_info.start_time
            dir_info.progress = 100
            dir_info.size_copied = dir_info.size
            
            if dir_info.duration > 0:
                dir_info.average_speed = dir_info.size / dir_info.duration
            
            self.session.last_completed_dir = dir_info
            self.session.completed_size += dir_info.size
            
            self._save_state()
        
        self.session.state = "stopped"
        self.session.current_dir = None
        self.session.next_dir = None
        self._save_state()
    
    async def _monitor_parallel_progress(self):
        """Monitor progress of parallel backup tasks"""
        # This would poll agents for status updates
        # For now, just wait and mark as complete
        await asyncio.sleep(5)  # Simulate work
        
        for dir_info in self.session.directories:
            if dir_info.status == "assigned":
                dir_info.status = "completed"
                dir_info.progress = 100
                dir_info.size_copied = dir_info.size
        
        self.session.state = "stopped"
        self._save_state()
    
    async def _simulate_backup(self, dir_info: DirectoryInfo):
        """Simulate backup progress (replace with actual rsync)"""
        for progress in range(0, 101, 25):
            if self.session.state != "running":
                break
            
            dir_info.progress = progress
            dir_info.size_copied = int(dir_info.size * progress / 100)
            self._save_state()
            await asyncio.sleep(1)
    
    def _find_next_directory(self, current_dir: DirectoryInfo) -> Optional[DirectoryInfo]:
        """Find the next directory to backup"""
        current_index = -1
        
        # Find current directory index
        for i, dir_info in enumerate(self.session.directories):
            if dir_info.name == current_dir.name:
                current_index = i
                break
        
        # Find next selected directory
        for i in range(current_index + 1, len(self.session.directories)):
            dir_info = self.session.directories[i]
            if dir_info.selected and dir_info.status == "pending":
                return dir_info
        
        return None
    
    def pause_backup(self):
        """Pause the backup process"""
        if self.session:
            self.session.state = "paused"
            self._save_state()
    
    def stop_backup(self):
        """Stop the backup process"""
        if self.session:
            self.session.state = "stopped"
            self.session.current_dir = None
            self.session.next_dir = None
            self._save_state()
    
    def get_status(self) -> Dict[str, Any]:
        """Get current backup status"""
        if not self.session:
            return {"error": "No active session"}
        
        return {
            "session_id": self.session.session_id,
            "directories": [self._directory_to_dict(d) for d in self.session.directories],
            "currentIndex": self._get_current_index(),
            "totalSize": self.session.total_size,
            "completedSize": self.session.completed_size,
            "startTime": int(self.session.start_time * 1000) if self.session.start_time else None,
            "state": self.session.state,
            "currentDir": self._directory_to_dict(self.session.current_dir) if self.session.current_dir else None,
            "lastCompletedDir": self._directory_to_dict(self.session.last_completed_dir) if self.session.last_completed_dir else None,
            "nextDir": self._directory_to_dict(self.session.next_dir) if self.session.next_dir else None,
            "errors": self.session.errors,
            "logs": self.logs[-100:],  # Last 100 logs
            "speedHistory": self.speed_history[-60:],  # Last 60 speed measurements
            "profiles": self.profiles,
            "activeProfile": self.session.active_profile,
            "history": self.history[:10]  # Last 10 history entries
        }
    
    def _directory_to_dict(self, dir_info: DirectoryInfo) -> Dict[str, Any]:
        """Convert DirectoryInfo to dictionary"""
        if not dir_info:
            return None
        
        return {
            "name": dir_info.name,
            "path": dir_info.path,
            "size": dir_info.size,
            "status": dir_info.status,
            "progress": dir_info.progress,
            "selected": dir_info.selected,
            "startTime": dir_info.start_time,
            "endTime": dir_info.end_time,
            "duration": dir_info.duration,
            "filesProcessed": dir_info.files_processed,
            "sizeCopied": dir_info.size_copied,
            "fileCount": dir_info.file_count,
            "averageSpeed": dir_info.average_speed,
            "assignedAgent": dir_info.assigned_agent
        }
    
    def _get_current_index(self) -> int:
        """Get index of current directory"""
        if not self.session.current_dir:
            return 0
        
        for i, dir_info in enumerate(self.session.directories):
            if dir_info.name == self.session.current_dir.name:
                return i
        
        return 0
    
    def _save_state(self):
        """Save current state to file"""
        try:
            with self.lock:
                with open(self.config.backup_status_file, 'w') as f:
                    json.dump(self.get_status(), f, indent=2)
        except Exception as e:
            print(f"Error saving state: {e}")


# Global instance
backup_manager = ModularBackupManager()


def get_backup_manager() -> ModularBackupManager:
    """Get the global backup manager instance"""
    return backup_manager