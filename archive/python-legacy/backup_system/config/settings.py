#!/usr/bin/env python3
"""
Configuration management for the backup system.
"""

import os
import time
from dataclasses import dataclass
from typing import List, Dict, Any


@dataclass
class BackupConfig:
    """Configuration settings for the backup system"""
    
    # File paths
    backup_status_file: str = "backup_status.json"
    backup_list_file: str = "backup_directories.txt"
    backup_history_file: str = "backup_history.json"
    profiles_file: str = "profiles.json"
    
    # Backup destination
    backup_dest_base: str = "/mnt/chromeos/removable/PNYRP60PSSD"
    backup_dest: str = None
    
    # Server settings
    port: int = 8888
    host: str = "localhost"
    
    # Logging
    log_dir: str = "logs"
    log_file: str = None
    max_log_size: int = 10 * 1024 * 1024  # 10MB
    backup_count: int = 10
    
    # Backup settings
    max_workers: int = 3
    max_logs: int = 1000
    rsync_excludes: List[str] = None
    
    # Performance settings
    update_interval: int = 1  # seconds
    progress_update_interval: int = 1  # seconds
    
    def __post_init__(self):
        """Initialize derived settings"""
        if self.backup_dest is None:
            date_str = time.strftime("%Y%m%d")
            self.backup_dest = f"{self.backup_dest_base}/pixelbook_backup_{date_str}"
        
        if self.log_file is None:
            from datetime import datetime
            log_filename = f"backup_{datetime.now().strftime('%Y%m%d')}.log"
            self.log_file = os.path.join(self.log_dir, log_filename)
        
        if self.rsync_excludes is None:
            self.rsync_excludes = [
                'venv', '.venv', 'env', '.env',
                'node_modules', '__pycache__', '*.pyc',
                '.git/objects', 'dist', 'build',
                '.next', '.cache', '*.log',
                '*.tmp', '*.swp'
            ]
    
    @classmethod
    def from_env(cls) -> 'BackupConfig':
        """Create configuration from environment variables"""
        config = cls()
        
        # Override with environment variables if present
        if os.getenv('BACKUP_PORT'):
            config.port = int(os.getenv('BACKUP_PORT'))
        
        if os.getenv('BACKUP_DEST'):
            config.backup_dest = os.getenv('BACKUP_DEST')
        
        if os.getenv('BACKUP_MAX_WORKERS'):
            config.max_workers = int(os.getenv('BACKUP_MAX_WORKERS'))
        
        if os.getenv('BACKUP_LOG_DIR'):
            config.log_dir = os.getenv('BACKUP_LOG_DIR')
        
        return config
    
    @classmethod
    def from_file(cls, config_file: str) -> 'BackupConfig':
        """Load configuration from JSON file"""
        import json
        
        try:
            with open(config_file, 'r') as f:
                data = json.load(f)
            
            # Create config with loaded data
            config = cls()
            for key, value in data.items():
                if hasattr(config, key):
                    setattr(config, key, value)
            
            # Re-run post_init to update derived settings
            config.__post_init__()
            return config
        
        except FileNotFoundError:
            # Return default config if file doesn't exist
            return cls()
    
    def save_to_file(self, config_file: str):
        """Save configuration to JSON file"""
        import json
        from dataclasses import asdict
        
        with open(config_file, 'w') as f:
            json.dump(asdict(self), f, indent=2)
    
    def validate(self) -> List[str]:
        """Validate configuration and return list of errors"""
        errors = []
        
        # Check if backup destination base exists
        if not os.path.exists(self.backup_dest_base):
            errors.append(f"Backup destination base does not exist: {self.backup_dest_base}")
        
        # Check port range
        if not (1024 <= self.port <= 65535):
            errors.append(f"Port must be between 1024 and 65535, got {self.port}")
        
        # Check max_workers
        if self.max_workers < 1:
            errors.append(f"max_workers must be at least 1, got {self.max_workers}")
        
        # Create log directory if it doesn't exist
        if not os.path.exists(self.log_dir):
            try:
                os.makedirs(self.log_dir, exist_ok=True)
            except PermissionError:
                errors.append(f"Cannot create log directory: {self.log_dir}")
        
        return errors


# Global configuration instance
config = BackupConfig.from_env()


def get_config() -> BackupConfig:
    """Get the global configuration instance"""
    return config


def load_config(config_file: str = None) -> BackupConfig:
    """Load configuration from file or environment"""
    global config
    
    if config_file and os.path.exists(config_file):
        config = BackupConfig.from_file(config_file)
    else:
        config = BackupConfig.from_env()
    
    return config


def validate_config() -> List[str]:
    """Validate the current configuration"""
    return config.validate()