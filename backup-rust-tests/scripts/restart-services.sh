#!/bin/bash

echo "Stopping existing services..."

# Kill any existing npm/node processes
pkill -f "npm run dev"
pkill -f "vite"
pkill -f "esbuild"

# Kill any existing cargo processes
pkill -f "cargo run"
pkill -f "backup-rust"

# Wait a moment
sleep 2

echo "Starting backend..."
cd ../backup-rust
cargo run --release &
BACKEND_PID=$!

# Wait for backend to start
echo "Waiting for backend to start on port 8888..."
sleep 5

# Check if backend is running
if ! curl -s http://localhost:8888/api/status > /dev/null; then
    echo "Backend failed to start"
    exit 1
fi

echo "Starting frontend..."  
cd ../backup-frontend
npm run dev &
FRONTEND_PID=$!

# Wait for frontend to start
echo "Waiting for frontend to start on port 3000..."
sleep 5

# Check if frontend is running
if ! curl -s http://localhost:3000 > /dev/null; then
    echo "Frontend failed to start"
    exit 1
fi

echo "Services started successfully"
echo "Backend PID: $BACKEND_PID"
echo "Frontend PID: $FRONTEND_PID"
echo "Backend: http://localhost:8888"
echo "Frontend: http://localhost:3000"