#!/bin/bash

# USB drive mount point
MOUNT_BASE="/mnt/chromeos/removable/PNYRP60PSSD"

# Check if USB drive is mounted
if [ ! -d "$MOUNT_BASE" ]; then
    echo "Error: USB drive not found at $MOUNT_BASE"
    echo "Please connect the USB drive and ensure it's mounted."
    exit 1
fi

# Additional check: verify it's actually a mount point
if ! mountpoint -q "$MOUNT_BASE"; then
    echo "Error: $MOUNT_BASE exists but is not a mount point"
    echo "Please ensure the USB drive is properly mounted."
    exit 1
fi

# Backup destination
BACKUP_ROOT="$MOUNT_BASE/pixelbook_backup_$(date +%Y%m%d)"
mkdir -p "$BACKUP_ROOT"

# Common exclusions for all directories
COMMON_EXCLUDES="--exclude='venv' --exclude='.venv' --exclude='env' --exclude='.env' --exclude='virtualenv' --exclude='node_modules' --exclude='__pycache__' --exclude='*.pyc' --exclude='.git/objects' --exclude='dist' --exclude='build' --exclude='.next' --exclude='.cache' --exclude='*.log' --exclude='*.tmp' --exclude='*.swp'"

# Function to backup a directory
backup_dir() {
    local SOURCE="$1"
    local DEST="$BACKUP_ROOT/$2"
    
    echo "Backing up $SOURCE to $DEST..."
    mkdir -p "$DEST"
    
    eval rsync -avzP $COMMON_EXCLUDES "$SOURCE/" "$DEST/"
}

# Backup important directories one by one
# Uncomment the ones you want to backup

# Documents and Downloads
backup_dir "$HOME/Documents" "Documents"
backup_dir "$HOME/Downloads" "Downloads"

# Project directories (add your important ones)
# backup_dir "$HOME/CLAUDE-CODE-CORE-MASTER-PROMPTS" "CLAUDE-CODE-CORE-MASTER-PROMPTS"
# backup_dir "$HOME/az1-website-001" "az1-website-001"
# backup_dir "$HOME/RESEARCH" "RESEARCH"
# backup_dir "$HOME/PROMPTS" "PROMPTS"

# Configuration files (excluding large caches)
backup_dir "$HOME/.config" ".config"
backup_dir "$HOME/.ssh" ".ssh"

# Scripts and small tools
# backup_dir "$HOME/scripts" "scripts"

echo "Backup completed to $BACKUP_ROOT"