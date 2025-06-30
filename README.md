# Backup Operations System v3.0

A high-performance, multi-threaded backup system built with Rust backend and TypeScript/React frontend. Provides real-time monitoring and control of rsync backup operations with dynamic resource scaling.

## Features

- **High Performance**: Built in Rust for optimal speed and resource efficiency
- **Dynamic Scaling**: Automatically adjusts worker count based on CPU/memory availability
- **Parallel Processing**: Multiple directories backed up simultaneously
- **Real-time Monitoring**: WebSocket updates with current file, progress, and speed
- **Rsync Integration**: Full monitoring with connection checks, verification, and metrics
- **Professional UI**: Modern grey-themed dashboard built with React and Chakra UI
- **Mount Verification**: Automatic checking if USB drive is properly mounted
- **Visual Progress**: Progress bars, circular progress, and speed graphs
- **Directory Selection**: Choose specific directories with sorting and filtering
- **State Persistence**: Backup progress saved across sessions
- **Comprehensive Logging**: Structured logs with levels and real-time filtering

## Requirements

- Rust 1.75+ (for backend)
- Node.js 18+ (for frontend)
- rsync
- USB drive mounted at `/mnt/chromeos/removable/PNYRP60PSSD/`

## Installation

1. Clone this repository:
```bash
git clone https://github.com/halcasteel/backup2usb-chromeos.git
cd backup2usb-chromeos
```

2. Build the Rust backend:
```bash
cd backup-rust
cargo build --release
```

3. Install frontend dependencies:
```bash
cd ../backup-frontend
npm install
```

4. Ensure your USB drive is mounted at the expected location

## Usage

### Using the Web Interface (Recommended)

1. Start the Rust backend:
```bash
cd backup-rust
cargo run --release
```

2. In another terminal, start the frontend:
```bash
cd backup-frontend
npm run dev
```

3. Open your browser to: http://localhost:3000

4. Select directories or choose a profile
5. Click START to begin backup

### Using the Command Line Script

For manual backups without the web interface:
```bash
./backup_folders.sh
```

Note: Edit the script to uncomment directories you want to backup.

## Web Interface Tabs

- **Backup**: Main control panel with directory selection and progress monitoring
- **Logs**: Real-time rsync output with filtering for errors/warnings
- **Schedule**: Set up automated backups (coming soon)
- **History**: View past backup sessions

## Directory Profiles

- **Development**: Code projects, config files, SSH keys
- **Documents**: Documents, Downloads, Pictures, Videos  
- **Media**: Pictures, Videos, Music, Downloads
- **Full Backup**: All non-hidden directories

## Safety Features

- Mount verification prevents backups to local filesystem
- Dry run mode for testing
- Real-time error reporting
- Skips common cache/temp directories (node_modules, venv, etc.)

## Architecture

### Backend (Rust)
- `backup-rust/src/api/`: REST API endpoints
- `backup-rust/src/backup/`: Core backup engine with dynamic task management
- `backup-rust/src/backup/rsync_monitor.rs`: Rsync monitoring and verification
- `backup-rust/src/storage/`: SQLite database for persistence
- `backup-rust/src/web/`: WebSocket server for real-time updates

### Frontend (TypeScript/React)
- `backup-frontend/src/components/`: React components (BackupTab, LogsTab, etc.)
- `backup-frontend/src/hooks/`: Custom hooks including WebSocket connection
- `backup-frontend/src/services/`: API client services
- `backup-frontend/src/store/`: Zustand state management

### Configuration
- `backup_status.json`: Current backup state
- `backup_history.json`: Historical backup records
- `profiles.json`: Backup profile definitions

## API Endpoints

- `GET /status`: Get current backup status with all metrics
- `POST /start`: Start backup operation
- `POST /pause`: Pause backup operation  
- `POST /stop`: Stop backup operation
- `GET /api/logs`: Get filtered logs
- `POST /api/profile`: Apply backup profile
- `POST /api/schedule`: Configure scheduled backups
- `WS /ws`: WebSocket for real-time updates

## Customization

To change the backup destination, edit:
```python
BACKUP_DEST = "/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup_" + time.strftime("%Y%m%d")
```

## License

MIT

## Contributing

Pull requests welcome! Please test changes with both dry run and actual backups.