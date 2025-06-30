#!/usr/bin/env python3
"""
File and directory utilities for the backup system.
"""

import os
import subprocess
import time
from typing import List, Dict, Any, Optional


def get_directory_size(path: str) -> int:
    """Get directory size in bytes using du command"""
    try:
        result = subprocess.run(['du', '-sb', path], capture_output=True, text=True)
        if result.returncode == 0:
            return int(result.stdout.split()[0])
    except Exception:
        pass
    return 0


def format_size(bytes_size: int) -> str:
    """Format bytes to human readable string"""
    if bytes_size < 1024:
        return f"{bytes_size} B"
    elif bytes_size < 1024 * 1024:
        return f"{bytes_size / 1024:.1f} KB"
    elif bytes_size < 1024 * 1024 * 1024:
        return f"{bytes_size / 1024 / 1024:.1f} MB"
    else:
        return f"{bytes_size / 1024 / 1024 / 1024:.2f} GB"


def format_duration(seconds: float) -> str:
    """Format duration in seconds to human readable string"""
    if seconds < 60:
        return f"{int(seconds)}s"
    elif seconds < 3600:
        return f"{int(seconds // 60)}m {int(seconds % 60)}s"
    else:
        return f"{int(seconds // 3600)}h {int((seconds % 3600) // 60)}m"


def get_disk_usage(path: str) -> Dict[str, int]:
    """Get disk usage information for a path"""
    try:
        import shutil
        total, used, free = shutil.disk_usage(path)
        return {
            'total': total,
            'used': used,
            'free': free,
            'percentage': (used / total * 100) if total > 0 else 0
        }
    except Exception:
        return {'total': 0, 'used': 0, 'free': 0, 'percentage': 0}


def is_mount_point(path: str) -> bool:
    """Check if path is a mount point"""
    try:
        return os.path.ismount(path)
    except Exception:
        return False


def discover_home_directories(exclude_hidden: bool = True) -> List[Dict[str, Any]]:
    """Discover directories in the home folder"""
    home = os.path.expanduser("~")
    directories = []
    
    try:
        for item in os.listdir(home):
            if exclude_hidden and item.startswith('.'):
                continue
            
            path = os.path.join(home, item)
            if os.path.isdir(path):
                try:
                    size = get_directory_size(path)
                    directories.append({
                        'name': item,
                        'path': path,
                        'size': size,
                        'type': 'directory'
                    })
                except Exception:
                    pass
        
        # Sort by name descending
        directories.sort(key=lambda x: x['name'], reverse=True)
        
        # Add important dot directories if requested
        if not exclude_hidden:
            for dot_dir in ['.ssh', '.config', '.gnupg']:
                path = os.path.join(home, dot_dir)
                if os.path.exists(path):
                    size = get_directory_size(path)
                    directories.append({
                        'name': dot_dir,
                        'path': path,
                        'size': size,
                        'type': 'hidden_directory'
                    })
    
    except Exception as e:
        print(f"Error discovering directories: {e}")
    
    return directories


def ensure_directory_exists(path: str) -> bool:
    """Ensure a directory exists, create if necessary"""
    try:
        os.makedirs(path, exist_ok=True)
        return True
    except Exception:
        return False


def safe_file_write(file_path: str, content: str, backup: bool = True) -> bool:
    """Safely write content to a file with optional backup"""
    try:
        # Create backup if requested and file exists
        if backup and os.path.exists(file_path):
            backup_path = f"{file_path}.backup.{int(time.time())}"
            import shutil
            shutil.copy2(file_path, backup_path)
        
        # Write to temporary file first
        temp_path = f"{file_path}.tmp"
        with open(temp_path, 'w') as f:
            f.write(content)
        
        # Atomic move
        os.rename(temp_path, file_path)
        return True
    
    except Exception:
        # Clean up temp file if it exists
        try:
            os.unlink(f"{file_path}.tmp")
        except Exception:
            pass
        return False


def get_file_count_estimate(path: str) -> int:
    """Get an estimate of file count in a directory"""
    try:
        result = subprocess.run(['find', path, '-type', 'f'], 
                              capture_output=True, text=True)
        if result.returncode == 0:
            return len(result.stdout.strip().split('\n'))
    except Exception:
        pass
    return 0