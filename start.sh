#!/bin/bash

# Backup System v3.0.0 Launch Script
# High-performance backup system with Rust backend and React frontend

set -e

echo "🚀 Starting Backup System v3.0.0"
echo "================================"

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Check if the release binary exists
if [ ! -f "./target/release/backup-system" ]; then
    echo "❌ Release binary not found!"
    echo "Building the system in release mode... (this may take a few minutes)"
    cd backup-rust
    cargo build --release
    cd ..
    if [ ! -f "./target/release/backup-system" ]; then
        echo "❌ Build failed! Please check the error messages above."
        exit 1
    fi
fi

# Create database file if it doesn't exist
if [ ! -f "./backup_system.db" ]; then
    touch ./backup_system.db
fi

# Check if port 8888 is already in use
if lsof -Pi :8888 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "⚠️  Warning: Port 8888 is already in use!"
    echo "Another instance might be running. Kill it? (y/n)"
    read -r response
    if [ "$response" = "y" ]; then
        pkill -f backup-system 2>/dev/null || true
        sleep 2
    else
        echo "Exiting..."
        exit 1
    fi
fi

echo ""
echo "✅ Server Configuration:"
echo "   URL: http://localhost:8888"
echo "   Home: $HOME"
echo "   Database: backup_system.db"
echo ""
echo "🔍 Mount Status:"
# Check if USB is mounted
if mountpoint -q "/mnt/chromeos/removable/PNYRP60PSSD" 2>/dev/null; then
    echo "   ✅ USB drive is mounted and ready"
    echo "   Path: /mnt/chromeos/removable/PNYRP60PSSD"
else
    echo "   ⚠️  USB drive NOT mounted"
    echo "   Expected path: /mnt/chromeos/removable/PNYRP60PSSD"
    echo "   Please mount your USB drive before starting backups!"
fi
echo ""
echo "📊 System will scan directories on startup..."
echo "   This may take a moment for large home directories"
echo ""
echo "Press Ctrl+C to stop the server"
echo "================================"
echo ""

# Start the server
./target/release/backup-system &
BACKEND_PID=$!

# Wait a moment for server to start
sleep 2

# Check if server started successfully
if curl -s http://localhost:8888/api/status > /dev/null 2>&1; then
    echo "✅ Server started successfully!"
    echo "🌐 Open http://localhost:8888 in your browser"
    echo ""
else
    echo "❌ Server failed to start. Check the logs above."
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

# Trap Ctrl+C and clean shutdown
trap "echo ''; echo 'Shutting down server...'; kill $BACKEND_PID 2>/dev/null || true; echo 'Server stopped.'; exit" INT TERM

# Keep script running
wait $BACKEND_PID