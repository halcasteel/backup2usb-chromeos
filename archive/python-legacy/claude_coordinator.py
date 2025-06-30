#!/usr/bin/env python3
"""
Claude Coordination System
Manages file checkout/checkin for multiple Claude sessions
"""
import json
import os
import time
import fcntl
from datetime import datetime
import logging

COORDINATION_FILE = "claude_coordination.json"
COORDINATION_LOG = "logs/claude_coordination.log"

class ClaudeCoordinator:
    def __init__(self):
        self.session_id = f"claude-{os.getpid()}-{int(time.time())}"
        self.setup_logging()
        
    def setup_logging(self):
        """Setup coordination logging"""
        os.makedirs("logs", exist_ok=True)
        self.logger = logging.getLogger('claude_coordinator')
        self.logger.setLevel(logging.INFO)
        
        handler = logging.FileHandler(COORDINATION_LOG)
        formatter = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
        handler.setFormatter(formatter)
        self.logger.addHandler(handler)
    
    def _load_state(self):
        """Load current coordination state"""
        if os.path.exists(COORDINATION_FILE):
            try:
                with open(COORDINATION_FILE, 'r') as f:
                    return json.load(f)
            except:
                pass
        return {"checkouts": {}, "sessions": {}}
    
    def _save_state(self, state):
        """Save coordination state atomically"""
        temp_file = f"{COORDINATION_FILE}.tmp"
        with open(temp_file, 'w') as f:
            json.dump(state, f, indent=2)
        os.rename(temp_file, COORDINATION_FILE)
    
    def checkout(self, filename, purpose="editing"):
        """Check out a file for editing"""
        max_retries = 30
        retry_count = 0
        
        while retry_count < max_retries:
            state = self._load_state()
            
            # Check if file is already checked out
            if filename in state["checkouts"]:
                checkout_info = state["checkouts"][filename]
                checkout_time = datetime.fromisoformat(checkout_info["timestamp"])
                age_minutes = (datetime.now() - checkout_time).total_seconds() / 60
                
                # If checkout is older than 30 minutes, consider it stale
                if age_minutes > 30:
                    self.logger.warning(f"Removing stale checkout for {filename} by {checkout_info['session_id']}")
                else:
                    self.logger.info(f"File {filename} is checked out by {checkout_info['session_id']} for {checkout_info['purpose']}")
                    time.sleep(1)
                    retry_count += 1
                    continue
            
            # Check out the file
            state["checkouts"][filename] = {
                "session_id": self.session_id,
                "timestamp": datetime.now().isoformat(),
                "purpose": purpose
            }
            
            # Update session info
            state["sessions"][self.session_id] = {
                "last_seen": datetime.now().isoformat(),
                "files": list(set(state["sessions"].get(self.session_id, {}).get("files", []) + [filename]))
            }
            
            self._save_state(state)
            self.logger.info(f"[CHECKOUT] {self.session_id} checked out {filename} for {purpose}")
            return True
        
        self.logger.error(f"Failed to check out {filename} after {max_retries} attempts")
        return False
    
    def checkin(self, filename, changes_made=""):
        """Check in a file after editing"""
        state = self._load_state()
        
        if filename in state["checkouts"]:
            if state["checkouts"][filename]["session_id"] == self.session_id:
                del state["checkouts"][filename]
                self._save_state(state)
                self.logger.info(f"[CHECKIN] {self.session_id} checked in {filename}. Changes: {changes_made}")
                return True
            else:
                self.logger.warning(f"Cannot check in {filename} - checked out by different session")
                return False
        else:
            self.logger.warning(f"File {filename} was not checked out")
            return True
    
    def get_status(self):
        """Get current checkout status"""
        state = self._load_state()
        return state
    
    def cleanup_session(self):
        """Clean up this session's checkouts"""
        state = self._load_state()
        
        # Find and remove all checkouts by this session
        files_to_remove = []
        for filename, info in state["checkouts"].items():
            if info["session_id"] == self.session_id:
                files_to_remove.append(filename)
        
        for filename in files_to_remove:
            del state["checkouts"][filename]
            self.logger.info(f"[CLEANUP] Released {filename}")
        
        # Remove session info
        if self.session_id in state["sessions"]:
            del state["sessions"][self.session_id]
        
        self._save_state(state)
    
    def announce(self, message):
        """Announce a message to other Claude sessions via log"""
        self.logger.info(f"[ANNOUNCE] {self.session_id}: {message}")
    
    def read_announcements(self, since_minutes=5):
        """Read recent announcements from other sessions"""
        announcements = []
        if os.path.exists(COORDINATION_LOG):
            with open(COORDINATION_LOG, 'r') as f:
                for line in f:
                    if "[ANNOUNCE]" in line:
                        try:
                            timestamp_str = line.split(' - ')[0]
                            timestamp = datetime.strptime(timestamp_str, '%Y-%m-%d %H:%M:%S,%f')
                            if (datetime.now() - timestamp).total_seconds() < since_minutes * 60:
                                announcements.append(line.strip())
                        except:
                            pass
        return announcements


# Example usage in backup_server.py modifications
def safe_edit_file(coordinator, filename, edit_function, purpose="updating"):
    """Safely edit a file with coordination"""
    if coordinator.checkout(filename, purpose):
        try:
            # Perform the edit
            result = edit_function()
            coordinator.checkin(filename, f"Completed {purpose}")
            return result
        except Exception as e:
            coordinator.checkin(filename, f"Failed {purpose}: {str(e)}")
            raise
    else:
        raise Exception(f"Could not acquire lock for {filename}")


if __name__ == "__main__":
    # Example usage
    coord = ClaudeCoordinator()
    
    # Check current status
    print("Current checkouts:")
    status = coord.get_status()
    for file, info in status["checkouts"].items():
        print(f"  {file}: {info['session_id']} ({info['purpose']})")
    
    # Try to check out a file
    if coord.checkout("backup_server.py", "fixing file count display"):
        print("Successfully checked out backup_server.py")
        # Do work...
        time.sleep(2)
        coord.checkin("backup_server.py", "Added save_status() call after updating filesProcessed")
    
    # Announce to other sessions
    coord.announce("Working on file count display issue - will modify backup_server.py lines 530-565")
    
    # Read recent announcements
    print("\nRecent announcements:")
    for announcement in coord.read_announcements():
        print(f"  {announcement}")
    
    # Cleanup
    coord.cleanup_session()