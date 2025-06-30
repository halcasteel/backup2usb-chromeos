# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a high-performance backup management system that provides a web-based interface for backing up directories from a Linux/Chrome OS system to external storage. The system uses a Rust backend for optimal performance and a React/TypeScript frontend for a modern user experience.

## Architecture

### Core Components

1. **Rust Backend (backup-rust/)** - High-performance backup server
   - Runs on port 8888 with Axum web framework
   - Multi-threaded with dynamic worker scaling based on system resources
   - Handles backup process lifecycle (start/pause/stop)
   - Real-time rsync monitoring with metrics and verification
   - REST API endpoints: `/api/status`, `/start`, `/pause`, `/stop`
   - WebSocket endpoint: `/ws` for real-time updates
   - SQLite database for state persistence

2. **React Frontend (backup-frontend/)** - Modern web dashboard
   - Real-time progress display with WebSocket updates
   - Professional grey theme with Chakra UI components
   - Directory selection with profiles and filtering
   - Visual progress bars, circular progress, and speed graphs
   - Tabbed interface: Backup, Logs, Schedule, History

3. **State Management**
   - SQLite database for persistent storage
   - Zero-copy message passing between workers
   - Lock-free data structures for high performance
   - Automatic state recovery on restart

### Key Technical Details

- **Backup destination**: `/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup_YYYYMMDD`
- **Excluded patterns**: venv, node_modules, __pycache__, .git/objects, dist, build, .cache, logs, temp files
- **Process management**: Single-threaded subprocess with proper signal handling
- **State persistence**: JSON file updated in real-time during backup operations

## Development Commands

### Running the Rust Backend
```bash
cd backup-rust
cargo run --release
```
The server will start on http://localhost:8888

### Running the React Frontend
```bash
cd backup-frontend
npm run dev
```
The frontend will start on http://localhost:3000

### Building for Production
```bash
# Backend
cd backup-rust
cargo build --release

# Frontend
cd backup-frontend
npm run build
```

### Testing Server Endpoints
```bash
# Check status
curl http://localhost:8888/api/status

# Start backup
curl -X POST http://localhost:8888/start

# Pause backup
curl -X POST http://localhost:8888/pause

# Stop backup
curl -X POST http://localhost:8888/stop
```

### Manual Backup Script
```bash
./backup_folders.sh
```
Note: Most directories are commented out by default. Edit the script to enable specific directories.

## Important Notes

- The system processes directories in parallel with multiple workers
- Dynamic worker scaling based on CPU cores and available memory
- Real-time progress tracking via rsync output parsing and WebSocket updates
- State persists across server restarts via SQLite database
- Graceful shutdown handling ensures clean termination of all workers
- **Mount verification**: Automatic checking if USB drive is properly mounted at `/mnt/chromeos/removable/PNYRP60PSSD` before starting backups
  - Verifies the path exists and is an actual mount point
  - Prevents accidental backups to local filesystem
  - Logs errors if drive is not mounted

## TODO

- Implement actual backend functionality for:
  - Logs collection and filtering
  - Profile management and persistence
  - Directory selection API
  - Dry run mode
  - Schedule management
- Add frontend UI integration for above features
- Implement disk space monitoring
- Add backup history tracking
- Test rsync integration with real backup operations
- Add progress persistence across restarts
- Implement worker pool management UI