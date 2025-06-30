#!/usr/bin/env python3
"""
Agent-to-Agent protocol implementation for backup workers.
Based on A2A project concepts for secure agent coordination.
"""

import json
import time
import uuid
from typing import Dict, List, Any, Optional, Callable
from dataclasses import dataclass, asdict
from enum import Enum
import asyncio
import aiohttp
from aiohttp import web


class AgentCapability(Enum):
    BACKUP_SEQUENTIAL = "backup.sequential"
    BACKUP_PARALLEL = "backup.parallel"
    BACKUP_INCREMENTAL = "backup.incremental"
    BACKUP_VERIFY = "backup.verify"
    COMPRESS = "compress"
    ENCRYPT = "encrypt"
    MONITOR = "monitor"
    COORDINATE = "coordinate"


class MessageType(Enum):
    DISCOVER = "discover"
    HANDOFF = "handoff"
    STATUS = "status"
    COMPLETE = "complete"
    ERROR = "error"
    HEARTBEAT = "heartbeat"


@dataclass
class AgentCard:
    """Agent capability card following A2A protocol"""
    agent_id: str
    name: str
    capabilities: List[AgentCapability]
    endpoint: str
    version: str = "1.0"
    max_concurrent_tasks: int = 1
    supported_formats: List[str] = None
    load_score: float = 0.0  # 0.0 = idle, 1.0 = fully loaded
    
    def __post_init__(self):
        if self.supported_formats is None:
            self.supported_formats = ["rsync", "tar", "zip"]


@dataclass
class TaskHandoff:
    """Task handoff request between agents"""
    task_id: str
    source_agent: str
    target_agent: str
    task_type: str
    payload: Dict[str, Any]
    priority: int = 1
    deadline: Optional[float] = None
    metadata: Dict[str, Any] = None


@dataclass
class A2AMessage:
    """Standard A2A message format (JSON-RPC 2.0 style)"""
    id: str
    method: str
    params: Dict[str, Any]
    timestamp: float
    sender: str
    receiver: Optional[str] = None
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "jsonrpc": "2.0",
            "id": self.id,
            "method": self.method,
            "params": self.params,
            "timestamp": self.timestamp,
            "sender": self.sender,
            "receiver": self.receiver
        }


class BackupAgent:
    """
    Backup worker agent that implements A2A protocol for coordination.
    Maintains opacity by not exposing internal state.
    """
    
    def __init__(self, agent_id: str, name: str, port: int, 
                 capabilities: List[AgentCapability]):
        self.agent_id = agent_id
        self.name = name
        self.port = port
        self.capabilities = capabilities
        self.endpoint = f"http://localhost:{port}"
        
        # Internal state (not exposed via A2A)
        self._current_tasks: Dict[str, Any] = {}
        self._peer_agents: Dict[str, AgentCard] = {}
        self._message_handlers: Dict[str, Callable] = {}
        self._load_score = 0.0
        
        # Register message handlers
        self._register_handlers()
        
        # Web server for A2A communication
        self.app = web.Application()
        self._setup_routes()
    
    def _register_handlers(self):
        """Register message handlers for different A2A methods"""
        self._message_handlers.update({
            "discover": self._handle_discover,
            "handoff": self._handle_handoff,
            "status": self._handle_status,
            "complete": self._handle_complete,
            "error": self._handle_error,
            "heartbeat": self._handle_heartbeat
        })
    
    def _setup_routes(self):
        """Setup HTTP routes for A2A communication"""
        self.app.router.add_post('/a2a', self._handle_a2a_message)
        self.app.router.add_get('/agent-card', self._handle_agent_card)
        self.app.router.add_get('/health', self._handle_health)
    
    async def start_server(self):
        """Start the A2A communication server"""
        runner = web.AppRunner(self.app)
        await runner.setup()
        site = web.TCPSite(runner, 'localhost', self.port)
        await site.start()
        print(f"Agent {self.agent_id} listening on {self.endpoint}")
    
    def get_agent_card(self) -> AgentCard:
        """Get this agent's capability card"""
        return AgentCard(
            agent_id=self.agent_id,
            name=self.name,
            capabilities=self.capabilities,
            endpoint=self.endpoint,
            max_concurrent_tasks=3,
            load_score=self._load_score
        )
    
    async def discover_peers(self, discovery_endpoints: List[str]) -> List[AgentCard]:
        """Discover other agents in the system"""
        discovered = []
        
        async with aiohttp.ClientSession() as session:
            for endpoint in discovery_endpoints:
                try:
                    async with session.get(f"{endpoint}/agent-card") as resp:
                        if resp.status == 200:
                            card_data = await resp.json()
                            card = AgentCard(**card_data)
                            discovered.append(card)
                            self._peer_agents[card.agent_id] = card
                            print(f"Discovered agent: {card.name} ({card.agent_id})")
                except Exception as e:
                    print(f"Failed to discover agent at {endpoint}: {e}")
        
        return discovered
    
    async def send_message(self, target_agent: str, method: str, 
                          params: Dict[str, Any]) -> Dict[str, Any]:
        """Send A2A message to another agent"""
        target_card = self._peer_agents.get(target_agent)
        if not target_card:
            raise ValueError(f"Unknown target agent: {target_agent}")
        
        message = A2AMessage(
            id=str(uuid.uuid4()),
            method=method,
            params=params,
            timestamp=time.time(),
            sender=self.agent_id,
            receiver=target_agent
        )
        
        async with aiohttp.ClientSession() as session:
            async with session.post(
                f"{target_card.endpoint}/a2a",
                json=message.to_dict()
            ) as resp:
                if resp.status == 200:
                    return await resp.json()
                else:
                    raise Exception(f"A2A message failed: {resp.status}")
    
    async def handoff_task(self, task_id: str, target_agent: str, 
                          task_data: Dict[str, Any]) -> bool:
        """Hand off a task to another agent"""
        try:
            handoff = TaskHandoff(
                task_id=task_id,
                source_agent=self.agent_id,
                target_agent=target_agent,
                task_type="backup_directory",
                payload=task_data,
                priority=task_data.get('priority', 1)
            )
            
            response = await self.send_message(
                target_agent, 
                "handoff", 
                asdict(handoff)
            )
            
            success = response.get('result', {}).get('accepted', False)
            if success:
                print(f"Successfully handed off task {task_id} to {target_agent}")
                # Remove from our internal tracking
                self._current_tasks.pop(task_id, None)
                self._update_load_score()
            
            return success
            
        except Exception as e:
            print(f"Failed to handoff task {task_id}: {e}")
            return False
    
    def _update_load_score(self):
        """Update load score based on current tasks"""
        max_tasks = self.get_agent_card().max_concurrent_tasks
        current_load = len(self._current_tasks)
        self._load_score = min(1.0, current_load / max_tasks)
    
    async def find_best_agent_for_task(self, task_requirements: Dict[str, Any]) -> Optional[str]:
        """Find the best agent to handle a specific task"""
        required_capability = task_requirements.get('capability', AgentCapability.BACKUP_SEQUENTIAL)
        min_load = float('inf')
        best_agent = None
        
        for agent_id, card in self._peer_agents.items():
            # Check if agent has required capability
            if required_capability in card.capabilities:
                # Prefer agents with lower load
                if card.load_score < min_load:
                    min_load = card.load_score
                    best_agent = agent_id
        
        return best_agent
    
    # HTTP handlers for A2A protocol
    async def _handle_a2a_message(self, request):
        """Handle incoming A2A messages"""
        try:
            data = await request.json()
            method = data.get('method')
            params = data.get('params', {})
            message_id = data.get('id')
            
            if method in self._message_handlers:
                result = await self._message_handlers[method](params)
                return web.json_response({
                    "jsonrpc": "2.0",
                    "id": message_id,
                    "result": result
                })
            else:
                return web.json_response({
                    "jsonrpc": "2.0",
                    "id": message_id,
                    "error": {"code": -32601, "message": "Method not found"}
                }, status=400)
        
        except Exception as e:
            return web.json_response({
                "jsonrpc": "2.0",
                "id": data.get('id') if 'data' in locals() else None,
                "error": {"code": -32603, "message": str(e)}
            }, status=500)
    
    async def _handle_agent_card(self, request):
        """Return this agent's capability card"""
        return web.json_response(asdict(self.get_agent_card()))
    
    async def _handle_health(self, request):
        """Health check endpoint"""
        return web.json_response({
            "status": "healthy",
            "agent_id": self.agent_id,
            "load_score": self._load_score,
            "active_tasks": len(self._current_tasks)
        })
    
    # A2A Message handlers
    async def _handle_discover(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle discovery request"""
        return {
            "agent_card": asdict(self.get_agent_card()),
            "discovered_at": time.time()
        }
    
    async def _handle_handoff(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle task handoff request"""
        handoff = TaskHandoff(**params)
        
        # Check if we can accept the task
        max_tasks = self.get_agent_card().max_concurrent_tasks
        if len(self._current_tasks) >= max_tasks:
            return {"accepted": False, "reason": "At capacity"}
        
        # Check if we have required capabilities
        required_cap = handoff.payload.get('required_capability')
        if required_cap and required_cap not in self.capabilities:
            return {"accepted": False, "reason": "Missing capability"}
        
        # Accept the task
        self._current_tasks[handoff.task_id] = {
            "handoff": handoff,
            "status": "accepted",
            "accepted_at": time.time()
        }
        self._update_load_score()
        
        print(f"Accepted task handoff: {handoff.task_id} from {handoff.source_agent}")
        
        # Start processing the task (in background)
        asyncio.create_task(self._process_handed_off_task(handoff))
        
        return {"accepted": True, "estimated_duration": 300}  # 5 minutes estimate
    
    async def _handle_status(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle status request"""
        task_id = params.get('task_id')
        if task_id in self._current_tasks:
            task_info = self._current_tasks[task_id]
            return {
                "task_id": task_id,
                "status": task_info.get('status', 'unknown'),
                "progress": task_info.get('progress', 0)
            }
        else:
            return {"error": "Task not found"}
    
    async def _handle_complete(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle task completion notification"""
        task_id = params.get('task_id')
        if task_id in self._current_tasks:
            self._current_tasks.pop(task_id)
            self._update_load_score()
            print(f"Task {task_id} marked as complete")
        
        return {"acknowledged": True}
    
    async def _handle_error(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle error notification"""
        task_id = params.get('task_id')
        error_msg = params.get('error_message', 'Unknown error')
        
        if task_id in self._current_tasks:
            self._current_tasks[task_id]['status'] = 'error'
            self._current_tasks[task_id]['error'] = error_msg
            print(f"Task {task_id} failed: {error_msg}")
        
        return {"acknowledged": True}
    
    async def _handle_heartbeat(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle heartbeat from peer agents"""
        sender = params.get('sender')
        if sender in self._peer_agents:
            self._peer_agents[sender].load_score = params.get('load_score', 0.0)
        
        return {
            "agent_id": self.agent_id,
            "load_score": self._load_score,
            "timestamp": time.time()
        }
    
    async def _process_handed_off_task(self, handoff: TaskHandoff):
        """Process a task that was handed off to us"""
        task_id = handoff.task_id
        
        try:
            # Update task status
            if task_id in self._current_tasks:
                self._current_tasks[task_id]['status'] = 'in_progress'
                self._current_tasks[task_id]['started_at'] = time.time()
            
            # Simulate backup processing
            directory_path = handoff.payload.get('directory_path')
            directory_size = handoff.payload.get('directory_size', 0)
            
            print(f"Starting backup of {directory_path} (handed off from {handoff.source_agent})")
            
            # Progress simulation
            for progress in range(0, 101, 25):
                if task_id in self._current_tasks:
                    self._current_tasks[task_id]['progress'] = progress
                await asyncio.sleep(1)  # Simulate work
            
            # Mark as completed
            if task_id in self._current_tasks:
                self._current_tasks[task_id]['status'] = 'completed'
                self._current_tasks[task_id]['completed_at'] = time.time()
            
            # Notify source agent of completion
            await self.send_message(handoff.source_agent, "complete", {
                "task_id": task_id,
                "completed_by": self.agent_id,
                "bytes_processed": directory_size
            })
            
            print(f"Completed handed-off task: {task_id}")
            
        except Exception as e:
            # Notify source agent of error
            await self.send_message(handoff.source_agent, "error", {
                "task_id": task_id,
                "error_message": str(e),
                "failed_by": self.agent_id
            })
            
            print(f"Failed handed-off task {task_id}: {e}")
        
        finally:
            # Clean up
            self._current_tasks.pop(task_id, None)
            self._update_load_score()


# Example usage
async def main():
    """Example of A2A coordination between backup agents"""
    
    # Create agents with different capabilities
    coordinator = BackupAgent(
        "coordinator-1", 
        "Backup Coordinator",
        8801,
        [AgentCapability.COORDINATE, AgentCapability.MONITOR]
    )
    
    worker1 = BackupAgent(
        "worker-1",
        "Sequential Backup Worker", 
        8802,
        [AgentCapability.BACKUP_SEQUENTIAL, AgentCapability.COMPRESS]
    )
    
    worker2 = BackupAgent(
        "worker-2",
        "Parallel Backup Worker",
        8803, 
        [AgentCapability.BACKUP_PARALLEL, AgentCapability.ENCRYPT]
    )
    
    # Start all agents
    await coordinator.start_server()
    await worker1.start_server()
    await worker2.start_server()
    
    # Discovery phase
    discovery_endpoints = [
        "http://localhost:8801",
        "http://localhost:8802", 
        "http://localhost:8803"
    ]
    
    await coordinator.discover_peers(discovery_endpoints)
    await worker1.discover_peers(discovery_endpoints)
    await worker2.discover_peers(discovery_endpoints)
    
    # Simulate task handoff
    best_agent = await coordinator.find_best_agent_for_task({
        'capability': AgentCapability.BACKUP_SEQUENTIAL,
        'directory_size': 1000000
    })
    
    if best_agent:
        await coordinator.handoff_task("task-123", best_agent, {
            'directory_path': '/home/user/Documents',
            'directory_size': 1000000,
            'priority': 1,
            'required_capability': AgentCapability.BACKUP_SEQUENTIAL
        })
    
    # Keep running
    await asyncio.sleep(10)


if __name__ == "__main__":
    asyncio.run(main())