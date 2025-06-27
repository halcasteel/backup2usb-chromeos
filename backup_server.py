#!/usr/bin/env python3
import json
import os
import time
import subprocess
import threading
from http.server import HTTPServer, SimpleHTTPRequestHandler
from urllib.parse import urlparse
import signal
import sys

BACKUP_STATUS_FILE = "backup_status.json"
BACKUP_LIST_FILE = "backup_directories.txt"
BACKUP_DEST = "/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup_" + time.strftime("%Y%m%d")
PORT = 8888

class BackupManager:
    def __init__(self):
        self.status = {
            "directories": [],
            "currentIndex": 0,
            "totalSize": 0,
            "completedSize": 0,
            "startTime": None,
            "errors": [],
            "state": "stopped",  # stopped, running, paused
            "currentDir": None,
            "logs": [],  # Store rsync logs
            "speedHistory": [],  # Store speed measurements for graph
            "profiles": self.load_profiles(),  # Backup profiles
            "activeProfile": None,
            "dryRun": False,  # Dry run mode
            "destinations": [BACKUP_DEST],  # Support multiple destinations
            "schedule": None,  # Backup schedule
            "history": []  # Backup history
        }
        self.backup_process = None
        self.lock = threading.Lock()
        self.log_buffer = []  # Buffer for log entries
        self.max_logs = 1000  # Maximum number of log entries to keep
        self.load_directories()
        
    def load_directories(self):
        """Load and sort directories from home folder"""
        try:
            # Get all directories in home folder
            home = os.path.expanduser("~")
            dirs = []
            
            for item in os.listdir(home):
                path = os.path.join(home, item)
                if os.path.isdir(path) and not item.startswith('.'):
                    try:
                        size = self.get_dir_size(path)
                        dirs.append({
                            "name": item,
                            "path": path,
                            "size": size,
                            "status": "pending",
                            "progress": 0,
                            "selected": True,
                            "filesProcessed": 0,
                            "sizeCopied": 0
                        })
                    except:
                        pass
            
            # Sort by name descending
            dirs.sort(key=lambda x: x["name"], reverse=True)
            
            # Also include important dot directories
            dot_dirs = ['.ssh', '.config', '.gnupg']
            for d in dot_dirs:
                path = os.path.join(home, d)
                if os.path.exists(path):
                    size = self.get_dir_size(path)
                    dirs.append({
                        "name": d,
                        "path": path,
                        "size": size,
                        "status": "pending",
                        "progress": 0,
                        "filesProcessed": 0,
                        "sizeCopied": 0
                    })
            
            self.status["directories"] = dirs
            self.status["totalSize"] = sum(d["size"] for d in dirs)
            self.save_status()
            
        except Exception as e:
            print(f"Error loading directories: {e}")
    
    def get_dir_size(self, path):
        """Get directory size in bytes"""
        try:
            result = subprocess.run(['du', '-sb', path], capture_output=True, text=True)
            if result.returncode == 0:
                return int(result.stdout.split()[0])
        except:
            pass
        return 0
    
    def save_status(self):
        """Save current status to file"""
        with self.lock:
            with open(BACKUP_STATUS_FILE, 'w') as f:
                json.dump(self.status, f, indent=2)
    
    def add_log(self, message, directory):
        """Add a log entry"""
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        log_entry = {
            "timestamp": timestamp,
            "directory": directory,
            "message": message,
            "level": self.classify_log_level(message)
        }
        
        with self.lock:
            self.status["logs"].append(log_entry)
            # Keep only the last max_logs entries
            if len(self.status["logs"]) > self.max_logs:
                self.status["logs"] = self.status["logs"][-self.max_logs:]
    
    def classify_log_level(self, message):
        """Classify log message level"""
        message_lower = message.lower()
        if any(word in message_lower for word in ['error', 'fail', 'cannot', 'unable']):
            return 'error'
        elif any(word in message_lower for word in ['warning', 'warn', 'skip']):
            return 'warning'
        else:
            return 'info'
    
    def update_speed_history(self, speed_str, timestamp):
        """Update speed history for graph"""
        # Convert speed string to bytes/sec
        speed_bytes = self.parse_speed(speed_str)
        
        with self.lock:
            self.status["speedHistory"].append({
                "timestamp": timestamp,
                "speed": speed_bytes,
                "speedStr": speed_str
            })
            # Keep only last 60 seconds of data
            cutoff = timestamp - 60
            self.status["speedHistory"] = [
                s for s in self.status["speedHistory"] if s["timestamp"] > cutoff
            ]
    
    def parse_speed(self, speed_str):
        """Parse speed string to bytes/sec"""
        try:
            # Remove '/s' and parse number
            speed_str = speed_str.replace('/s', '')
            if 'KB' in speed_str:
                return float(speed_str.replace('KB', '')) * 1024
            elif 'MB' in speed_str:
                return float(speed_str.replace('MB', '')) * 1024 * 1024
            elif 'GB' in speed_str:
                return float(speed_str.replace('GB', '')) * 1024 * 1024 * 1024
            else:
                return float(speed_str.replace('B', ''))
        except:
            return 0
    
    def start_backup(self):
        """Start or resume backup process"""
        if self.status["state"] == "running":
            return
        
        self.status["state"] = "running"
        if self.status["startTime"] is None:
            self.status["startTime"] = int(time.time() * 1000)
        
        self.save_status()
        
        # Start backup in separate thread
        self.backup_thread = threading.Thread(target=self.run_backup)
        self.backup_thread.start()
    
    def run_backup(self):
        """Run the actual backup process"""
        # Check if USB drive is mounted
        mount_base = "/mnt/chromeos/removable/PNYRP60PSSD"
        if not os.path.exists(mount_base):
            error_msg = f"USB drive not mounted at {mount_base}"
            self.status["errors"].append({
                "directory": "System",
                "error": error_msg,
                "time": time.strftime("%Y-%m-%d %H:%M:%S")
            })
            self.status["state"] = "stopped"
            self.save_status()
            print(f"Error: {error_msg}")
            return
            
        # Additional check: verify it's actually a mount point, not just a directory
        if not os.path.ismount(mount_base):
            error_msg = f"Path exists but is not a mount point: {mount_base}"
            self.status["errors"].append({
                "directory": "System",
                "error": error_msg,
                "time": time.strftime("%Y-%m-%d %H:%M:%S")
            })
            self.status["state"] = "stopped"
            self.save_status()
            print(f"Error: {error_msg}")
            return
            
        os.makedirs(BACKUP_DEST, exist_ok=True)
        
        for i in range(self.status["currentIndex"], len(self.status["directories"])):
            if self.status["state"] != "running":
                break
                
            dir_info = self.status["directories"][i]
            
            # Skip unselected directories
            if not dir_info.get("selected", True):
                dir_info["status"] = "skipped"
                self.status["currentIndex"] = i + 1
                self.save_status()
                continue
            self.status["currentIndex"] = i
            self.status["currentDir"] = dir_info
            dir_info["status"] = "active"
            self.save_status()
            
            # Run rsync
            dest_path = os.path.join(BACKUP_DEST, dir_info["name"])
            cmd = [
                'rsync', '-avzP',
                '--exclude=venv', '--exclude=.venv', '--exclude=env', '--exclude=.env',
                '--exclude=node_modules', '--exclude=__pycache__', '--exclude=*.pyc',
                '--exclude=.git/objects', '--exclude=dist', '--exclude=build',
                '--exclude=.next', '--exclude=.cache', '--exclude=*.log',
                '--exclude=*.tmp', '--exclude=*.swp',
                '--info=progress2'
            ]
            
            # Add dry-run flag if enabled
            if self.status.get('dryRun', False):
                cmd.append('--dry-run')
            
            cmd.extend([
                dir_info["path"] + '/',
                dest_path + '/'
            ])
            
            try:
                self.backup_process = subprocess.Popen(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,  # Combine stderr with stdout
                    universal_newlines=True,
                    bufsize=1  # Line buffered
                )
                
                # Monitor progress
                start_time = time.time()
                last_speed_update = start_time
                
                for line in self.backup_process.stdout:
                    if self.status["state"] != "running":
                        self.backup_process.terminate()
                        break
                    
                    # Log the line
                    self.add_log(line.strip(), dir_info["name"])
                        
                    # Parse rsync progress
                    if '%' in line:
                        try:
                            parts = line.strip().split()
                            if len(parts) > 1 and '%' in parts[1]:
                                progress = int(parts[1].rstrip('%'))
                                dir_info["progress"] = progress
                                
                                # Update completed size
                                dir_info["sizeCopied"] = int(dir_info["size"] * progress / 100)
                                self.status["completedSize"] = sum(
                                    d["size"] if d["status"] == "completed" else d.get("sizeCopied", 0)
                                    for d in self.status["directories"]
                                )
                                
                                # Extract speed for graph
                                for part in parts:
                                    if '/s' in part and any(c in part for c in ['K', 'M', 'G', 'B']):
                                        current_time = time.time()
                                        if current_time - last_speed_update >= 1:
                                            self.update_speed_history(part, current_time)
                                            last_speed_update = current_time
                                        break
                                
                                self.save_status()
                        except:
                            pass
                
                self.backup_process.wait()
                
                if self.backup_process.returncode == 0:
                    dir_info["status"] = "completed"
                    dir_info["progress"] = 100
                    dir_info["sizeCopied"] = dir_info["size"]
                else:
                    dir_info["status"] = "error"
                    self.status["errors"].append(f"Error backing up {dir_info['name']}")
                    
            except Exception as e:
                dir_info["status"] = "error"
                self.status["errors"].append(f"Exception backing up {dir_info['name']}: {str(e)}")
            
            self.save_status()
        
        self.status["state"] = "stopped"
        self.status["currentDir"] = None
        self.save_status()
    
    def pause_backup(self):
        """Pause the backup process"""
        self.status["state"] = "paused"
        if self.backup_process:
            self.backup_process.terminate()
        self.save_status()
    
    def stop_backup(self):
        """Stop the backup process"""
        self.status["state"] = "stopped"
        if self.backup_process:
            self.backup_process.terminate()
        self.status["currentIndex"] = 0
        self.save_status()

class BackupHTTPHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        url = urlparse(self.path)
        
        if url.path == '/':
            self.send_response(200)
            self.send_header('Content-type', 'text/html')
            self.end_headers()
            with open('BACKUP-OPS-DASHBOARD-V2.html', 'rb') as f:
                self.wfile.write(f.read())
                
        elif url.path == '/api/status':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            with backup_manager.lock:
                self.wfile.write(json.dumps(backup_manager.status).encode())
                
        else:
            self.send_error(404)
    
    def do_POST(self):
        if self.path == '/api/control':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data)
            
            action = data.get('action')
            if action == 'start':
                backup_manager.start_backup()
            elif action == 'pause':
                backup_manager.pause_backup()
            elif action == 'stop':
                backup_manager.stop_backup()
            
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
            
        elif self.path == '/api/logs':
            # Get logs with optional filter
            content_length = int(self.headers['Content-Length']) if 'Content-Length' in self.headers else 0
            if content_length > 0:
                post_data = self.rfile.read(content_length)
                data = json.loads(post_data)
                filter_level = data.get('level', None)
            else:
                filter_level = None
            
            with backup_manager.lock:
                logs = backup_manager.status['logs']
                if filter_level:
                    logs = [log for log in logs if log['level'] == filter_level]
            
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"logs": logs}).encode())
            
        elif self.path == '/api/profile':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data)
            
            profile_id = data.get('profile')
            if profile_id and profile_id in backup_manager.status['profiles']:
                with backup_manager.lock:
                    backup_manager.status['activeProfile'] = profile_id
                    profile = backup_manager.status['profiles'][profile_id]
                    
                    # Update directory selection based on profile
                    if profile['directories']:  # If not empty (full backup)
                        for dir_info in backup_manager.status['directories']:
                            dir_info['selected'] = dir_info['name'] in profile['directories']
                    else:
                        # Full backup - select all
                        for dir_info in backup_manager.status['directories']:
                            dir_info['selected'] = True
                    
                    backup_manager.save_status()
            
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
            
        elif self.path == '/api/select':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data)
            
            selected = data.get('selected', [])
            with backup_manager.lock:
                # Update selected status for all directories
                for dir_info in backup_manager.status['directories']:
                    dir_info['selected'] = dir_info['name'] in selected
                backup_manager.save_status()
            
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
            
        elif self.path == '/api/dryrun':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data)
            
            with backup_manager.lock:
                backup_manager.status['dryRun'] = data.get('enabled', False)
                backup_manager.save_status()
            
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
            
        else:
            self.send_error(404)
    
    def log_message(self, format, *args):
        pass  # Suppress request logging

def signal_handler(sig, frame):
    print("\nShutting down...")
    backup_manager.pause_backup()
    sys.exit(0)

if __name__ == "__main__":
    backup_manager = BackupManager()
    
    signal.signal(signal.SIGINT, signal_handler)
    
    print(f"Starting Backup Operations Dashboard on http://localhost:{PORT}")
    print("Press Ctrl+C to stop")
    
    httpd = HTTPServer(('localhost', PORT), BackupHTTPHandler)
    httpd.serve_forever()