# backup2usb-chromeos

A professional web-based backup management system for Chrome OS/Linux that provides real-time monitoring and control of rsync backup operations to USB drives.

## Features

- **Web Dashboard**: Modern, responsive interface with real-time updates
- **Visual Progress Tracking**: Progress bars, pie charts, and speed graphs
- **Directory Selection**: Choose specific directories to backup with checkboxes
- **Backup Profiles**: Pre-configured sets (Development, Documents, Media, Full)
- **Mount Verification**: Automatic checking if USB drive is properly mounted
- **Rsync Log Viewer**: Real-time logs with error/warning filtering
- **Dry Run Mode**: Preview what will be backed up without copying
- **Speed Monitoring**: Live transfer speed graph
- **Sorting Options**: Sort directories by name or size
- **State Persistence**: Backup progress saved across sessions

## Requirements

- Python 3.x
- rsync
- USB drive mounted at `/mnt/chromeos/removable/PNYRP60PSSD/`

## Installation

1. Clone this repository:
```bash
git clone https://github.com/halcasteel/backup2usb-chromeos.git
cd backup2usb-chromeos
```

2. Ensure your USB drive is mounted at the expected location, or modify the `BACKUP_DEST` in `backup_server.py`

## Usage

### Using the Web Interface (Recommended)

1. Start the backup server:
```bash
python3 backup_server.py
```

2. Open your browser to: http://localhost:8888

3. Select directories or choose a profile
4. Click START to begin backup

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

- `backup_server.py`: HTTP server managing backup operations
- `BACKUP-OPS-DASHBOARD-V2.html`: Web interface
- `backup_folders.sh`: Standalone bash script for manual backups
- `backup_status.json`: Persistent state storage

## API Endpoints

- `GET /api/status`: Get current backup status
- `POST /api/control`: Start/pause/stop backup
- `POST /api/select`: Update directory selection
- `POST /api/profile`: Apply backup profile
- `POST /api/dryrun`: Toggle dry run mode
- `POST /api/logs`: Get filtered logs

## Customization

To change the backup destination, edit:
```python
BACKUP_DEST = "/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup_" + time.strftime("%Y%m%d")
```

## License

MIT

## Contributing

Pull requests welcome! Please test changes with both dry run and actual backups.