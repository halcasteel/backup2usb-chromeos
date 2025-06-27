# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a backup management system that provides a web-based interface for backing up directories from a Linux/Chrome OS system to external storage. The system uses Python for the server backend and provides real-time progress monitoring through a web dashboard.

## Architecture

### Core Components

1. **backup_server.py** - HTTP server that manages backup operations
   - Runs on port 8888
   - Handles backup process lifecycle (start/pause/stop)
   - Tracks progress by parsing rsync output
   - Provides REST endpoints: `/status`, `/start`, `/pause`, `/stop`
   - Automatically discovers directories in home folder

2. **BACKUP-OPS-DASHBOARD.html** - Web interface for monitoring backups
   - Real-time progress display with speed and ETA
   - Directory list with status indicators
   - Error logging display
   - Dark theme UI

3. **backup_status.json** - Persistent state storage
   - Tracks directory list sorted by name (descending)
   - Progress for each directory
   - Total size calculations
   - Error logs

### Key Technical Details

- **Backup destination**: `/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup_YYYYMMDD`
- **Excluded patterns**: venv, node_modules, __pycache__, .git/objects, dist, build, .cache, logs, temp files
- **Process management**: Single-threaded subprocess with proper signal handling
- **State persistence**: JSON file updated in real-time during backup operations

## Development Commands

### Running the Server
```bash
python3 backup_server.py
```
The server will start on http://localhost:8888

### Manual Backup Script
```bash
./backup_folders.sh
```
Note: Most directories are commented out by default. Edit the script to enable specific directories.

### Testing Server Endpoints
```bash
# Check status
curl http://localhost:8888/status

# Start backup
curl http://localhost:8888/start

# Pause backup
curl http://localhost:8888/pause

# Stop backup
curl http://localhost:8888/stop
```

## Important Notes

- The system processes directories sequentially, not in parallel
- Progress is tracked by parsing rsync's output for percentage completion
- The dashboard polls for updates every second
- Backup state persists across server restarts via backup_status.json
- Signal handling (SIGINT) ensures clean shutdown of rsync processes
- **Mount verification**: Both scripts now check if the USB drive is properly mounted at `/mnt/chromeos/removable/PNYRP60PSSD` before starting backups
  - Verifies the path exists and is an actual mount point
  - Prevents accidental backups to local filesystem
  - Logs errors to backup_status.json if drive is not mounted