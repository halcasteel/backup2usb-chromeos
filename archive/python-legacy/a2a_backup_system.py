#!/usr/bin/env python3
"""
A2A (Agent-to-Agent) Backup System
Distributed backup system with coordinated agents, task management, and resource monitoring
"""
import asyncio
import json
import logging
import os
import psutil
import threading
import time
import uuid
from dataclasses import dataclass, asdict
from datetime import datetime, timedelta
from enum import Enum
from typing import Dict, List, Optional, Any
import aiohttp
from aiohttp import web
import subprocess

# Agent Types and Task Definitions
class AgentType(Enum):
    COORDINATOR = "coordinator"
    WORKER = "worker"
    MONITOR = "monitor"
    SCHEDULER = "scheduler"

class TaskType(Enum):
    BACKUP_DIRECTORY = "backup_directory"
    VERIFY_BACKUP = "verify_backup"
    CLEANUP = "cleanup"
    HEALTH_CHECK = "health_check"
    RESOURCE_MONITOR = "resource_monitor"

class TaskStatus(Enum):
    PENDING = "pending"
    ASSIGNED = "assigned"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"

@dataclass
class Task:
    id: str
    type: TaskType
    priority: int
    payload: Dict[str, Any]
    status: TaskStatus = TaskStatus.PENDING
    assigned_agent: Optional[str] = None
    created_at: datetime = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    progress: float = 0.0
    result: Optional[Dict[str, Any]] = None
    error: Optional[str] = None
    
    def __post_init__(self):
        if self.created_at is None:
            self.created_at = datetime.now()

@dataclass
class AgentInfo:
    id: str
    type: AgentType
    host: str
    port: int
    capabilities: List[str]
    max_concurrent_tasks: int = 2
    current_tasks: int = 0
    cpu_usage: float = 0.0
    memory_usage: float = 0.0
    last_heartbeat: datetime = None
    status: str = "active"
    
    def __post_init__(self):
        if self.last_heartbeat is None:
            self.last_heartbeat = datetime.now()

class ResourceMonitor:
    """Monitor system resources and enforce limits"""
    
    def __init__(self, max_cpu_percent=80, max_memory_percent=85):
        self.max_cpu_percent = max_cpu_percent
        self.max_memory_percent = max_memory_percent
        self.logger = logging.getLogger(f"{__name__}.ResourceMonitor")
    
    def get_system_stats(self):
        """Get current system resource usage"""
        return {
            "cpu_percent": psutil.cpu_percent(interval=1),
            "memory_percent": psutil.virtual_memory().percent,
            "disk_usage": psutil.disk_usage('/').percent,
            "load_average": os.getloadavg()[0] if hasattr(os, 'getloadavg') else 0,
            "timestamp": datetime.now().isoformat()
        }
    
    def can_start_task(self, task_type: TaskType) -> bool:
        """Check if system resources allow starting a new task"""
        stats = self.get_system_stats()
        
        # Heavy tasks require more resources
        if task_type in [TaskType.BACKUP_DIRECTORY, TaskType.VERIFY_BACKUP]:
            return (stats["cpu_percent"] < self.max_cpu_percent and 
                   stats["memory_percent"] < self.max_memory_percent)
        
        # Light tasks can run with higher resource usage
        return (stats["cpu_percent"] < 95 and 
               stats["memory_percent"] < 95)

class TaskManager:
    """Central task coordination and distribution"""
    
    def __init__(self):
        self.tasks: Dict[str, Task] = {}
        self.agents: Dict[str, AgentInfo] = {}
        self.task_queue: List[str] = []
        self.resource_monitor = ResourceMonitor()
        self.logger = logging.getLogger(f"{__name__}.TaskManager")
        self._lock = asyncio.Lock()
    
    async def register_agent(self, agent_info: AgentInfo):
        """Register a new agent"""
        async with self._lock:
            self.agents[agent_info.id] = agent_info
            self.logger.info(f"Registered agent {agent_info.id} of type {agent_info.type}")
    
    async def create_task(self, task_type: TaskType, payload: Dict[str, Any], priority: int = 5) -> str:
        """Create a new task"""
        task_id = str(uuid.uuid4())
        task = Task(
            id=task_id,
            type=task_type,
            priority=priority,
            payload=payload
        )
        
        async with self._lock:
            self.tasks[task_id] = task
            self.task_queue.append(task_id)
            self.task_queue.sort(key=lambda tid: self.tasks[tid].priority, reverse=True)
        
        self.logger.info(f"Created task {task_id} of type {task_type}")
        return task_id
    
    async def assign_tasks(self):
        """Assign pending tasks to available agents"""
        async with self._lock:
            available_agents = [
                agent for agent in self.agents.values()
                if (agent.status == "active" and 
                   agent.current_tasks < agent.max_concurrent_tasks and
                   (datetime.now() - agent.last_heartbeat).seconds < 30)
            ]
            
            for task_id in self.task_queue[:]:
                task = self.tasks[task_id]
                if task.status != TaskStatus.PENDING:
                    continue
                
                # Find suitable agent
                suitable_agents = [
                    agent for agent in available_agents
                    if task.type.value in agent.capabilities and
                       self.resource_monitor.can_start_task(task.type)
                ]
                
                if suitable_agents:
                    # Choose agent with lowest current load
                    chosen_agent = min(suitable_agents, 
                                     key=lambda a: (a.current_tasks, a.cpu_usage))
                    
                    task.status = TaskStatus.ASSIGNED
                    task.assigned_agent = chosen_agent.id
                    chosen_agent.current_tasks += 1
                    self.task_queue.remove(task_id)
                    
                    # Notify agent about task assignment
                    await self._notify_agent(chosen_agent, task)
    
    async def _notify_agent(self, agent: AgentInfo, task: Task):
        """Notify agent about task assignment"""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"http://{agent.host}:{agent.port}/api/task",
                    json=asdict(task)
                ) as response:
                    if response.status == 200:
                        self.logger.info(f"Assigned task {task.id} to agent {agent.id}")
                    else:
                        self.logger.error(f"Failed to assign task {task.id} to agent {agent.id}")
                        # Reset task status
                        task.status = TaskStatus.PENDING
                        task.assigned_agent = None
                        agent.current_tasks -= 1
                        self.task_queue.append(task.id)
        except Exception as e:
            self.logger.error(f"Error notifying agent {agent.id}: {e}")
    
    async def update_task_progress(self, task_id: str, progress: float, result: Dict = None):
        """Update task progress"""
        if task_id in self.tasks:
            self.tasks[task_id].progress = progress
            if result:
                self.tasks[task_id].result = result

class BackupAgent:
    """Individual backup worker agent"""
    
    def __init__(self, agent_id: str, coordinator_url: str, port: int = 0):
        self.agent_id = agent_id
        self.coordinator_url = coordinator_url
        self.port = port
        self.capabilities = [
            TaskType.BACKUP_DIRECTORY.value,
            TaskType.VERIFY_BACKUP.value,
            TaskType.HEALTH_CHECK.value
        ]
        self.current_tasks: Dict[str, Task] = {}
        self.max_concurrent_tasks = 2
        self.logger = logging.getLogger(f"{__name__}.BackupAgent.{agent_id}")
        self.app = None
        self.runner = None
        
    async def start(self):
        """Start the agent server"""
        self.app = web.Application()
        self.app.router.add_post('/api/task', self.handle_task_assignment)
        self.app.router.add_get('/api/status', self.handle_status_request)
        self.app.router.add_post('/api/heartbeat', self.handle_heartbeat)
        
        self.runner = web.AppRunner(self.app)
        await self.runner.setup()
        site = web.TCPSite(self.runner, 'localhost', self.port)
        await site.start()
        
        # Get actual port if 0 was specified
        if self.port == 0:
            self.port = site._server.sockets[0].getsockname()[1]
        
        # Register with coordinator
        await self._register_with_coordinator()
        
        # Start heartbeat
        asyncio.create_task(self._heartbeat_loop())
        
        self.logger.info(f"Agent {self.agent_id} started on port {self.port}")
    
    async def _register_with_coordinator(self):
        """Register this agent with the coordinator"""
        agent_info = AgentInfo(
            id=self.agent_id,
            type=AgentType.WORKER,
            host="localhost",
            port=self.port,
            capabilities=self.capabilities,
            max_concurrent_tasks=self.max_concurrent_tasks
        )
        
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self.coordinator_url}/api/agents/register",
                    json=asdict(agent_info)
                ) as response:
                    if response.status == 200:
                        self.logger.info("Successfully registered with coordinator")
                    else:
                        self.logger.error("Failed to register with coordinator")
        except Exception as e:
            self.logger.error(f"Error registering with coordinator: {e}")
    
    async def _heartbeat_loop(self):
        """Send periodic heartbeat to coordinator"""
        while True:
            try:
                await asyncio.sleep(10)  # Heartbeat every 10 seconds
                
                stats = psutil.Process().as_dict(attrs=['cpu_percent', 'memory_percent'])
                heartbeat_data = {
                    "agent_id": self.agent_id,
                    "current_tasks": len(self.current_tasks),
                    "cpu_usage": stats.get('cpu_percent', 0),
                    "memory_usage": stats.get('memory_percent', 0),
                    "timestamp": datetime.now().isoformat()
                }
                
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        f"{self.coordinator_url}/api/agents/heartbeat",
                        json=heartbeat_data
                    ) as response:
                        if response.status != 200:
                            self.logger.warning("Heartbeat failed")
            except Exception as e:
                self.logger.error(f"Heartbeat error: {e}")
    
    async def handle_task_assignment(self, request):
        """Handle task assignment from coordinator"""
        try:
            task_data = await request.json()
            task = Task(**task_data)
            
            if len(self.current_tasks) >= self.max_concurrent_tasks:
                return web.json_response({"error": "Agent at capacity"}, status=503)
            
            self.current_tasks[task.id] = task
            asyncio.create_task(self._execute_task(task))
            
            return web.json_response({"status": "accepted"})
        except Exception as e:
            self.logger.error(f"Error handling task assignment: {e}")
            return web.json_response({"error": str(e)}, status=500)
    
    async def handle_status_request(self, request):
        """Handle status request"""
        return web.json_response({
            "agent_id": self.agent_id,
            "current_tasks": len(self.current_tasks),
            "capabilities": self.capabilities,
            "status": "active"
        })
    
    async def handle_heartbeat(self, request):
        """Handle heartbeat request"""
        return web.json_response({"status": "ok"})
    
    async def _execute_task(self, task: Task):
        """Execute a task"""
        try:
            task.status = TaskStatus.IN_PROGRESS
            task.started_at = datetime.now()
            
            if task.type == TaskType.BACKUP_DIRECTORY:
                await self._backup_directory(task)
            elif task.type == TaskType.VERIFY_BACKUP:
                await self._verify_backup(task)
            elif task.type == TaskType.HEALTH_CHECK:
                await self._health_check(task)
            else:
                raise ValueError(f"Unknown task type: {task.type}")
            
            task.status = TaskStatus.COMPLETED
            task.completed_at = datetime.now()
            task.progress = 100.0
            
        except Exception as e:
            task.status = TaskStatus.FAILED
            task.error = str(e)
            task.completed_at = datetime.now()
            self.logger.error(f"Task {task.id} failed: {e}")
        
        finally:
            # Notify coordinator of completion
            await self._notify_task_completion(task)
            del self.current_tasks[task.id]
    
    async def _backup_directory(self, task: Task):
        """Execute directory backup"""
        payload = task.payload
        source_path = payload["source_path"]
        dest_path = payload["dest_path"]
        
        self.logger.info(f"Starting backup: {source_path} -> {dest_path}")
        
        # Build rsync command
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
            source_path + '/',
            dest_path + '/'
        ]
        
        # Execute rsync with progress monitoring
        process = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT
        )
        
        files_processed = 0
        while True:
            line = await process.stdout.readline()
            if not line:
                break
            
            line_str = line.decode('utf-8').strip()
            
            # Extract progress information
            if 'xfr#' in line_str and '%' in line_str:
                try:
                    import re
                    match = re.search(r'xfr#(\d+)', line_str)
                    if match:
                        files_processed = int(match.group(1))
                        
                    # Extract percentage
                    match = re.search(r'(\d+)%', line_str)
                    if match:
                        progress = int(match.group(1))
                        task.progress = progress
                        
                        # Update coordinator with progress
                        await self._notify_progress(task, progress, {
                            "files_processed": files_processed,
                            "current_file": line_str
                        })
                except:
                    pass
        
        await process.wait()
        
        if process.returncode == 0:
            task.result = {
                "files_processed": files_processed,
                "status": "success",
                "source_path": source_path,
                "dest_path": dest_path
            }
        else:
            raise Exception(f"Rsync failed with return code {process.returncode}")
    
    async def _verify_backup(self, task: Task):
        """Verify backup integrity"""
        payload = task.payload
        source_path = payload["source_path"]
        dest_path = payload["dest_path"]
        
        self.logger.info(f"Verifying backup: {source_path} vs {dest_path}")
        
        cmd = [
            'rsync', '-avzn', '--checksum', '--itemize-changes',
            '--exclude=venv', '--exclude=.venv', '--exclude=env', '--exclude=.env',
            '--exclude=node_modules', '--exclude=__pycache__', '--exclude=*.pyc',
            '--exclude=.git/objects', '--exclude=dist', '--exclude=build',
            source_path + '/',
            dest_path + '/'
        ]
        
        process = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )
        
        stdout, stderr = await process.communicate()
        
        differences = []
        for line in stdout.decode().split('\n'):
            if line and not line.startswith('sending') and not line.startswith('sent'):
                if line.startswith('>'):
                    differences.append(line)
        
        task.result = {
            "verified": len(differences) == 0,
            "differences": differences,
            "source_path": source_path,
            "dest_path": dest_path
        }
    
    async def _health_check(self, task: Task):
        """Perform health check"""
        stats = psutil.Process().as_dict(attrs=[
            'cpu_percent', 'memory_percent', 'create_time'
        ])
        
        task.result = {
            "agent_id": self.agent_id,
            "cpu_percent": stats.get('cpu_percent', 0),
            "memory_percent": stats.get('memory_percent', 0),
            "uptime": time.time() - stats.get('create_time', time.time()),
            "current_tasks": len(self.current_tasks),
            "status": "healthy"
        }
    
    async def _notify_progress(self, task: Task, progress: float, data: Dict):
        """Notify coordinator of task progress"""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self.coordinator_url}/api/tasks/{task.id}/progress",
                    json={"progress": progress, "data": data}
                ) as response:
                    pass  # Don't log every progress update
        except Exception as e:
            self.logger.warning(f"Failed to update progress: {e}")
    
    async def _notify_task_completion(self, task: Task):
        """Notify coordinator of task completion"""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self.coordinator_url}/api/tasks/{task.id}/complete",
                    json=asdict(task)
                ) as response:
                    if response.status == 200:
                        self.logger.info(f"Task {task.id} completion reported")
        except Exception as e:
            self.logger.error(f"Failed to report task completion: {e}")

class BackupCoordinator:
    """Main coordinator for A2A backup system"""
    
    def __init__(self, port: int = 8889):
        self.port = port
        self.task_manager = TaskManager()
        self.logger = logging.getLogger(f"{__name__}.BackupCoordinator")
        self.app = None
        self.runner = None
    
    async def start(self):
        """Start the coordinator server"""
        self.app = web.Application()
        
        # API routes
        self.app.router.add_post('/api/agents/register', self.handle_agent_registration)
        self.app.router.add_post('/api/agents/heartbeat', self.handle_agent_heartbeat)
        self.app.router.add_post('/api/tasks/create', self.handle_task_creation)
        self.app.router.add_post('/api/tasks/{task_id}/progress', self.handle_task_progress)
        self.app.router.add_post('/api/tasks/{task_id}/complete', self.handle_task_completion)
        self.app.router.add_get('/api/status', self.handle_status_request)
        
        self.runner = web.AppRunner(self.app)
        await self.runner.setup()
        site = web.TCPSite(self.runner, 'localhost', self.port)
        await site.start()
        
        # Start task assignment loop
        asyncio.create_task(self._task_assignment_loop())
        
        self.logger.info(f"Coordinator started on port {self.port}")
    
    async def _task_assignment_loop(self):
        """Continuously assign tasks to agents"""
        while True:
            try:
                await asyncio.sleep(2)  # Check every 2 seconds
                await self.task_manager.assign_tasks()
            except Exception as e:
                self.logger.error(f"Error in task assignment loop: {e}")
    
    async def handle_agent_registration(self, request):
        """Handle agent registration"""
        try:
            agent_data = await request.json()
            agent_info = AgentInfo(**agent_data)
            await self.task_manager.register_agent(agent_info)
            return web.json_response({"status": "registered"})
        except Exception as e:
            self.logger.error(f"Error registering agent: {e}")
            return web.json_response({"error": str(e)}, status=500)
    
    async def handle_agent_heartbeat(self, request):
        """Handle agent heartbeat"""
        try:
            heartbeat_data = await request.json()
            agent_id = heartbeat_data["agent_id"]
            
            if agent_id in self.task_manager.agents:
                agent = self.task_manager.agents[agent_id]
                agent.last_heartbeat = datetime.now()
                agent.current_tasks = heartbeat_data.get("current_tasks", 0)
                agent.cpu_usage = heartbeat_data.get("cpu_usage", 0)
                agent.memory_usage = heartbeat_data.get("memory_usage", 0)
            
            return web.json_response({"status": "ok"})
        except Exception as e:
            self.logger.error(f"Error handling heartbeat: {e}")
            return web.json_response({"error": str(e)}, status=500)
    
    async def handle_task_creation(self, request):
        """Handle task creation"""
        try:
            task_data = await request.json()
            task_id = await self.task_manager.create_task(
                TaskType(task_data["type"]),
                task_data["payload"],
                task_data.get("priority", 5)
            )
            return web.json_response({"task_id": task_id})
        except Exception as e:
            self.logger.error(f"Error creating task: {e}")
            return web.json_response({"error": str(e)}, status=500)
    
    async def handle_task_progress(self, request):
        """Handle task progress update"""
        try:
            task_id = request.match_info['task_id']
            progress_data = await request.json()
            await self.task_manager.update_task_progress(
                task_id,
                progress_data["progress"],
                progress_data.get("data")
            )
            return web.json_response({"status": "updated"})
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def handle_task_completion(self, request):
        """Handle task completion"""
        try:
            task_id = request.match_info['task_id']
            task_data = await request.json()
            
            if task_id in self.task_manager.tasks:
                task = self.task_manager.tasks[task_id]
                task.status = TaskStatus(task_data["status"])
                task.completed_at = datetime.fromisoformat(task_data["completed_at"])
                task.progress = task_data["progress"]
                task.result = task_data.get("result")
                task.error = task_data.get("error")
                
                # Update agent task count
                if task.assigned_agent in self.task_manager.agents:
                    self.task_manager.agents[task.assigned_agent].current_tasks -= 1
            
            return web.json_response({"status": "updated"})
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def handle_status_request(self, request):
        """Handle status request"""
        return web.json_response({
            "agents": {aid: asdict(agent) for aid, agent in self.task_manager.agents.items()},
            "tasks": {tid: asdict(task) for tid, task in self.task_manager.tasks.items()},
            "queue_length": len(self.task_manager.task_queue),
            "resource_stats": self.task_manager.resource_monitor.get_system_stats()
        })

# Example usage and startup script
async def main():
    """Main entry point for A2A backup system"""
    import sys
    
    if len(sys.argv) < 2:
        print("Usage: python a2a_backup_system.py [coordinator|agent] [options]")
        return
    
    mode = sys.argv[1]
    
    if mode == "coordinator":
        coordinator = BackupCoordinator(port=8889)
        await coordinator.start()
        print("Coordinator running on http://localhost:8889")
        
        # Keep running
        try:
            while True:
                await asyncio.sleep(1)
        except KeyboardInterrupt:
            print("Shutting down coordinator...")
    
    elif mode == "agent":
        agent_id = sys.argv[2] if len(sys.argv) > 2 else f"agent-{uuid.uuid4().hex[:8]}"
        coordinator_url = "http://localhost:8889"
        
        agent = BackupAgent(agent_id, coordinator_url)
        await agent.start()
        print(f"Agent {agent_id} running and registered with coordinator")
        
        # Keep running
        try:
            while True:
                await asyncio.sleep(1)
        except KeyboardInterrupt:
            print(f"Shutting down agent {agent_id}...")

if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    asyncio.run(main())