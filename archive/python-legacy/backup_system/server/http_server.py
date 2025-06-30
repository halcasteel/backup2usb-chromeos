#!/usr/bin/env python3
"""
HTTP server for the backup system API.
Separated from business logic for better modularity.
"""

import json
import os
import asyncio
from typing import Dict, Any
from aiohttp import web, WSMsgType
from aiohttp.web import Request, Response, WebSocketResponse
import aiohttp_cors
import weakref

from ..core.backup_manager import get_backup_manager
from ..config.settings import get_config


class BackupHTTPServer:
    """HTTP server handling REST API and WebSocket connections"""
    
    def __init__(self):
        self.config = get_config()
        self.backup_manager = get_backup_manager()
        self.app = web.Application()
        self.websockets = weakref.WeakSet()
        
        self._setup_routes()
        self._setup_cors()
    
    def _setup_routes(self):
        """Setup HTTP routes"""
        # Static file serving
        self.app.router.add_get('/', self._serve_dashboard)
        self.app.router.add_static('/static/', path='backup_system/web/static/', name='static')
        
        # REST API endpoints
        api_routes = [
            ('GET', '/api/status', self._handle_status),
            ('POST', '/api/control', self._handle_control),
            ('POST', '/api/profile', self._handle_profile),
            ('POST', '/api/select', self._handle_select),
            ('POST', '/api/dryrun', self._handle_dryrun),
            ('POST', '/api/schedule', self._handle_schedule),
            ('POST', '/api/logs', self._handle_logs),
            ('GET', '/api/logs/download', self._handle_logs_download),
            ('GET', '/api/logs/list', self._handle_logs_list),
            ('GET', '/api/health', self._handle_health),
        ]
        
        for method, path, handler in api_routes:
            self.app.router.add_route(method, path, handler)
        
        # WebSocket endpoint for real-time updates
        self.app.router.add_get('/ws', self._handle_websocket)
    
    def _setup_cors(self):
        """Setup CORS for cross-origin requests"""
        cors = aiohttp_cors.setup(self.app, defaults={
            "*": aiohttp_cors.ResourceOptions(
                allow_credentials=True,
                expose_headers="*",
                allow_headers="*",
                allow_methods="*"
            )
        })
        
        # Add CORS to all routes
        for route in list(self.app.router.routes()):
            cors.add(route)
    
    async def _serve_dashboard(self, request: Request) -> Response:
        """Serve the main dashboard HTML"""
        try:
            dashboard_path = os.path.join('backup_system', 'web', 'templates', 'dashboard.html')
            
            # Fallback to the current dashboard if new one doesn't exist
            if not os.path.exists(dashboard_path):
                dashboard_path = 'BACKUP-OPS-DASHBOARD-V2.html'
            
            with open(dashboard_path, 'r') as f:
                content = f.read()
            
            return web.Response(text=content, content_type='text/html')
        
        except FileNotFoundError:
            return web.Response(text="Dashboard not found", status=404)
    
    async def _handle_status(self, request: Request) -> Response:
        """Handle status API endpoint"""
        try:
            status = self.backup_manager.get_status()
            
            # Add disk space information
            status.update(await self._get_disk_space_info())
            
            return web.json_response(status)
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_control(self, request: Request) -> Response:
        """Handle backup control (start/pause/stop)"""
        try:
            data = await request.json()
            action = data.get('action')
            
            if action == 'start':
                use_parallel = data.get('parallel', True)
                success = await self.backup_manager.start_backup(use_parallel=use_parallel)
                if success:
                    await self._broadcast_to_websockets({"type": "backup_started"})
                return web.json_response({"status": "ok", "started": success})
            
            elif action == 'pause':
                self.backup_manager.pause_backup()
                await self._broadcast_to_websockets({"type": "backup_paused"})
                return web.json_response({"status": "ok"})
            
            elif action == 'stop':
                self.backup_manager.stop_backup()
                await self._broadcast_to_websockets({"type": "backup_stopped"})
                return web.json_response({"status": "ok"})
            
            else:
                return web.json_response({"error": "Invalid action"}, status=400)
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_profile(self, request: Request) -> Response:
        """Handle profile selection"""
        try:
            data = await request.json()
            profile_id = data.get('profile')
            
            # This would update the backup manager's profile selection
            # For now, just acknowledge
            return web.json_response({"status": "ok"})
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_select(self, request: Request) -> Response:
        """Handle directory selection"""
        try:
            data = await request.json()
            selected = data.get('selected', [])
            
            # Update directory selection in backup manager
            if self.backup_manager.session:
                for dir_info in self.backup_manager.session.directories:
                    dir_info.selected = dir_info.name in selected
                
                self.backup_manager._save_state()
            
            return web.json_response({"status": "ok"})
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_dryrun(self, request: Request) -> Response:
        """Handle dry run toggle"""
        try:
            data = await request.json()
            enabled = data.get('enabled', False)
            
            # This would configure dry run mode
            return web.json_response({"status": "ok"})
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_schedule(self, request: Request) -> Response:
        """Handle backup scheduling"""
        try:
            data = await request.json()
            
            # This would configure backup scheduling
            return web.json_response({"status": "ok"})
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_logs(self, request: Request) -> Response:
        """Handle log retrieval"""
        try:
            if request.method == 'POST':
                data = await request.json()
                filter_level = data.get('level')
            else:
                filter_level = None
            
            logs = self.backup_manager.logs
            
            if filter_level and filter_level != 'all':
                logs = [log for log in logs if log.get('level') == filter_level]
            
            return web.json_response({"logs": logs})
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_logs_download(self, request: Request) -> Response:
        """Handle log file download"""
        try:
            log_file = self.config.log_file
            
            if os.path.exists(log_file):
                with open(log_file, 'rb') as f:
                    content = f.read()
                
                filename = f"backup_{int(time.time())}.log"
                headers = {
                    'Content-Disposition': f'attachment; filename="{filename}"',
                    'Content-Type': 'text/plain'
                }
                
                return web.Response(body=content, headers=headers)
            else:
                return web.Response(text="Log file not found", status=404)
        
        except Exception as e:
            return web.Response(text=f"Error: {e}", status=500)
    
    async def _handle_logs_list(self, request: Request) -> Response:
        """Handle log file listing"""
        try:
            log_files = []
            
            if os.path.exists(self.config.log_dir):
                for filename in sorted(os.listdir(self.config.log_dir), reverse=True):
                    if filename.endswith('.log'):
                        filepath = os.path.join(self.config.log_dir, filename)
                        stat = os.stat(filepath)
                        log_files.append({
                            'filename': filename,
                            'size': stat.st_size,
                            'modified': stat.st_mtime
                        })
            
            return web.json_response({'logs': log_files})
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_health(self, request: Request) -> Response:
        """Handle health check"""
        try:
            # Check if agents are running
            agent_status = "unknown"
            if self.backup_manager.coordinator_agent:
                agent_status = "running"
            
            health_info = {
                "status": "healthy",
                "timestamp": time.time(),
                "backup_state": self.backup_manager.session.state if self.backup_manager.session else "no_session",
                "agent_status": agent_status,
                "websocket_connections": len(self.websockets)
            }
            
            return web.json_response(health_info)
        
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
    
    async def _handle_websocket(self, request: Request) -> WebSocketResponse:
        """Handle WebSocket connections for real-time updates"""
        ws = web.WebSocketResponse()
        await ws.prepare(request)
        
        # Add to active connections
        self.websockets.add(ws)
        
        try:
            # Send initial status
            status = self.backup_manager.get_status()
            await ws.send_str(json.dumps({
                "type": "status_update",
                "data": status
            }))
            
            # Handle incoming messages
            async for msg in ws:
                if msg.type == WSMsgType.TEXT:
                    try:
                        data = json.loads(msg.data)
                        await self._handle_websocket_message(ws, data)
                    except json.JSONDecodeError:
                        await ws.send_str(json.dumps({
                            "type": "error",
                            "message": "Invalid JSON"
                        }))
                elif msg.type == WSMsgType.ERROR:
                    print(f'WebSocket error: {ws.exception()}')
        
        except Exception as e:
            print(f"WebSocket error: {e}")
        
        return ws
    
    async def _handle_websocket_message(self, ws: WebSocketResponse, data: Dict[str, Any]):
        """Handle incoming WebSocket messages"""
        msg_type = data.get('type')
        
        if msg_type == 'get_status':
            status = self.backup_manager.get_status()
            await ws.send_str(json.dumps({
                "type": "status_update",
                "data": status
            }))
        
        elif msg_type == 'subscribe_updates':
            # Client wants to subscribe to updates
            await ws.send_str(json.dumps({
                "type": "subscribed",
                "message": "Subscribed to real-time updates"
            }))
    
    async def _broadcast_to_websockets(self, message: Dict[str, Any]):
        """Broadcast message to all connected WebSocket clients"""
        if not self.websockets:
            return
        
        message_str = json.dumps(message)
        
        # Send to all connected clients
        for ws in list(self.websockets):
            try:
                await ws.send_str(message_str)
            except Exception as e:
                print(f"Error sending to WebSocket: {e}")
                # WebSocket will be automatically removed from WeakSet
    
    async def _get_disk_space_info(self) -> Dict[str, Any]:
        """Get disk space information"""
        import shutil
        
        disk_info = {}
        
        try:
            # Remote (USB) disk space
            if os.path.exists(self.config.backup_dest_base):
                total, used, free = shutil.disk_usage(self.config.backup_dest_base)
                disk_info['remoteDiskSpace'] = {
                    'free': free,
                    'total': total,
                    'used': used,
                    'percentage': (used / total * 100) if total > 0 else 0
                }
            
            # Local disk space
            home_path = os.path.expanduser("~")
            total, used, free = shutil.disk_usage(home_path)
            disk_info['localDiskSpace'] = {
                'free': free,
                'total': total,
                'used': used,
                'percentage': (used / total * 100) if total > 0 else 0
            }
        
        except Exception as e:
            print(f"Error getting disk space: {e}")
        
        return disk_info
    
    async def start_server(self):
        """Start the HTTP server"""
        runner = web.AppRunner(self.app)
        await runner.setup()
        
        site = web.TCPSite(runner, self.config.host, self.config.port)
        await site.start()
        
        print(f"Backup HTTP server started on http://{self.config.host}:{self.config.port}")
        
        # Start background task for periodic WebSocket updates
        asyncio.create_task(self._periodic_updates())
    
    async def _periodic_updates(self):
        """Send periodic status updates to WebSocket clients"""
        while True:
            try:
                await asyncio.sleep(self.config.update_interval)
                
                if self.websockets:
                    status = self.backup_manager.get_status()
                    await self._broadcast_to_websockets({
                        "type": "status_update",
                        "data": status
                    })
            
            except Exception as e:
                print(f"Error in periodic updates: {e}")
                await asyncio.sleep(5)  # Wait before retrying


# Global server instance
http_server = BackupHTTPServer()


def get_http_server() -> BackupHTTPServer:
    """Get the global HTTP server instance"""
    return http_server