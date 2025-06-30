#!/usr/bin/env python3
"""
Main entry point for the modular backup system.

This replaces the monolithic backup_server.py with a proper modular architecture
using A2A (Agent-to-Agent) coordination between worker processes.
"""

import asyncio
import signal
import sys
import os
import logging
import logging.handlers
import time
from typing import Optional

# Add the current directory to Python path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from backup_system.config.settings import load_config, validate_config
from backup_system.core.backup_manager import get_backup_manager
from backup_system.server.http_server import get_http_server


class BackupSystemMain:
    """Main application controller"""
    
    def __init__(self):
        self.config = load_config()
        self.backup_manager = get_backup_manager()
        self.http_server = get_http_server()
        self.shutdown_event = asyncio.Event()
        
        # Setup logging
        self._setup_logging()
        self.logger = logging.getLogger(__name__)
    
    def _setup_logging(self):
        """Setup application logging"""
        # Create logs directory if it doesn't exist
        os.makedirs(self.config.log_dir, exist_ok=True)
        
        # Configure root logger
        logger = logging.getLogger()
        logger.setLevel(logging.DEBUG)
        
        # File handler with rotation
        file_handler = logging.handlers.RotatingFileHandler(
            self.config.log_file,
            maxBytes=self.config.max_log_size,
            backupCount=self.config.backup_count,
            encoding='utf-8'
        )
        file_handler.setLevel(logging.DEBUG)
        
        # Console handler
        console_handler = logging.StreamHandler()
        console_handler.setLevel(logging.INFO)
        
        # Formatter
        formatter = logging.Formatter(
            '%(asctime)s - %(name)s - %(levelname)s - %(message)s',
            datefmt='%Y-%m-%d %H:%M:%S'
        )
        file_handler.setFormatter(formatter)
        console_handler.setFormatter(formatter)
        
        # Add handlers
        logger.addHandler(file_handler)
        logger.addHandler(console_handler)
    
    async def startup(self):
        """Initialize and start all system components"""
        self.logger.info("Starting Backup System...")
        
        # Validate configuration
        config_errors = validate_config()
        if config_errors:
            self.logger.error("Configuration validation failed:")
            for error in config_errors:
                self.logger.error(f"  - {error}")
            return False
        
        try:
            # Initialize backup manager and agents
            self.logger.info("Initializing backup manager...")
            await self.backup_manager.initialize_agents(num_workers=self.config.max_workers)
            
            # Start HTTP server
            self.logger.info("Starting HTTP server...")
            await self.http_server.start_server()
            
            self.logger.info(f"Backup System started successfully!")
            self.logger.info(f"Dashboard: http://{self.config.host}:{self.config.port}")
            self.logger.info(f"API: http://{self.config.host}:{self.config.port}/api/")
            self.logger.info(f"WebSocket: ws://{self.config.host}:{self.config.port}/ws")
            
            return True
        
        except Exception as e:
            self.logger.error(f"Failed to start system: {e}", exc_info=True)
            return False
    
    async def shutdown(self):
        """Graceful shutdown of all components"""
        self.logger.info("Shutting down Backup System...")
        
        # Stop any running backups
        if self.backup_manager.session and self.backup_manager.session.state == "running":
            self.logger.info("Stopping running backup...")
            self.backup_manager.stop_backup()
        
        # Set shutdown event
        self.shutdown_event.set()
        
        self.logger.info("Backup System shutdown complete")
    
    def setup_signal_handlers(self):
        """Setup signal handlers for graceful shutdown"""
        def signal_handler(signum, frame):
            self.logger.info(f"Received signal {signum}, initiating shutdown...")
            asyncio.create_task(self.shutdown())
        
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)
    
    async def run(self):
        """Main application loop"""
        # Setup signal handlers
        self.setup_signal_handlers()
        
        # Start system
        if not await self.startup():
            return 1
        
        try:
            # Wait for shutdown signal
            await self.shutdown_event.wait()
        
        except KeyboardInterrupt:
            self.logger.info("Keyboard interrupt received")
        
        except Exception as e:
            self.logger.error(f"Unexpected error: {e}", exc_info=True)
            return 1
        
        finally:
            await self.shutdown()
        
        return 0


async def main():
    """Application entry point"""
    app = BackupSystemMain()
    return await app.run()


def cli_main():
    """CLI entry point for setuptools"""
    return asyncio.run(main())


if __name__ == "__main__":
    # Print startup banner
    print("=" * 60)
    print("🔄 BACKUP SYSTEM - Modular A2A Architecture")
    print("=" * 60)
    print()
    
    # Run the application
    exit_code = asyncio.run(main())
    sys.exit(exit_code)