#!/bin/bash
"""
A2A Backup System Startup Script
Starts coordinator and multiple backup agents with proper resource management
"""

set -e

# Configuration
COORDINATOR_PORT=8889
NUM_AGENTS=${NUM_AGENTS:-3}  # Default to 3 agents
LOG_DIR="logs/a2a"
PID_DIR="pids"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Create directories
mkdir -p "$LOG_DIR" "$PID_DIR"

# Function to log with timestamp
log() {
    echo -e "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

# Function to check if a process is running
is_running() {
    local pid_file="$1"
    if [[ -f "$pid_file" ]]; then
        local pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            return 0
        else
            rm -f "$pid_file"
            return 1
        fi
    fi
    return 1
}

# Function to stop all processes
stop_all() {
    log "${YELLOW}Stopping A2A backup system...${NC}"
    
    # Stop agents
    for i in $(seq 1 $NUM_AGENTS); do
        local pid_file="$PID_DIR/agent_$i.pid"
        if is_running "$pid_file"; then
            local pid=$(cat "$pid_file")
            log "Stopping agent $i (PID: $pid)"
            kill "$pid"
            rm -f "$pid_file"
        fi
    done
    
    # Stop coordinator
    local coord_pid_file="$PID_DIR/coordinator.pid"
    if is_running "$coord_pid_file"; then
        local pid=$(cat "$coord_pid_file")
        log "Stopping coordinator (PID: $pid)"
        kill "$pid"
        rm -f "$coord_pid_file"
    fi
    
    log "${GREEN}A2A backup system stopped${NC}"
}

# Function to start coordinator
start_coordinator() {
    local pid_file="$PID_DIR/coordinator.pid"
    local log_file="$LOG_DIR/coordinator.log"
    
    if is_running "$pid_file"; then
        log "${YELLOW}Coordinator already running${NC}"
        return 0
    fi
    
    log "${BLUE}Starting coordinator on port $COORDINATOR_PORT...${NC}"
    python3 a2a_backup_system.py coordinator > "$log_file" 2>&1 &
    local pid=$!
    echo "$pid" > "$pid_file"
    
    # Wait for coordinator to start
    sleep 3
    if is_running "$pid_file"; then
        log "${GREEN}Coordinator started (PID: $pid)${NC}"
        return 0
    else
        log "${RED}Failed to start coordinator${NC}"
        return 1
    fi
}

# Function to start agents
start_agents() {
    log "${BLUE}Starting $NUM_AGENTS backup agents...${NC}"
    
    for i in $(seq 1 $NUM_AGENTS); do
        local pid_file="$PID_DIR/agent_$i.pid"
        local log_file="$LOG_DIR/agent_$i.log"
        local agent_id="backup-agent-$i"
        
        if is_running "$pid_file"; then
            log "${YELLOW}Agent $i already running${NC}"
            continue
        fi
        
        log "Starting agent $i ($agent_id)..."
        python3 a2a_backup_system.py agent "$agent_id" > "$log_file" 2>&1 &
        local pid=$!
        echo "$pid" > "$pid_file"
        
        sleep 1
        if is_running "$pid_file"; then
            log "${GREEN}Agent $i started (PID: $pid)${NC}"
        else
            log "${RED}Failed to start agent $i${NC}"
        fi
    done
}

# Function to show status
show_status() {
    log "${BLUE}A2A Backup System Status:${NC}"
    
    # Check coordinator
    local coord_pid_file="$PID_DIR/coordinator.pid"
    if is_running "$coord_pid_file"; then
        local pid=$(cat "$coord_pid_file")
        log "${GREEN}✓ Coordinator running (PID: $pid)${NC}"
    else
        log "${RED}✗ Coordinator not running${NC}"
    fi
    
    # Check agents
    local running_agents=0
    for i in $(seq 1 $NUM_AGENTS); do
        local pid_file="$PID_DIR/agent_$i.pid"
        if is_running "$pid_file"; then
            local pid=$(cat "$pid_file")
            log "${GREEN}✓ Agent $i running (PID: $pid)${NC}"
            ((running_agents++))
        else
            log "${RED}✗ Agent $i not running${NC}"
        fi
    done
    
    log "Running agents: $running_agents/$NUM_AGENTS"
    
    # Show coordinator API status if running
    if is_running "$coord_pid_file"; then
        log "\nCoordinator API: http://localhost:$COORDINATOR_PORT/api/status"
        log "Dashboard integration: http://localhost:8888 (if backup server is running)"
    fi
}

# Function to create backup tasks
create_backup_tasks() {
    local directories_file="backup_directories.txt"
    
    if [[ ! -f "$directories_file" ]]; then
        log "${RED}No backup directories file found: $directories_file${NC}"
        return 1
    fi
    
    log "${BLUE}Creating backup tasks from $directories_file...${NC}"
    
    while IFS= read -r line || [[ -n "$line" ]]; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
        
        local source_path="$line"
        local dir_name=$(basename "$source_path")
        local dest_path="/mnt/chromeos/removable/PNYRP60PSSD/pixelbook_backup_$(date +%Y%m%d)/$dir_name"
        
        log "Creating backup task: $source_path -> $dest_path"
        
        # Create task via API
        curl -s -X POST "http://localhost:$COORDINATOR_PORT/api/tasks/create" \
            -H "Content-Type: application/json" \
            -d "{
                \"type\": \"backup_directory\",
                \"priority\": 5,
                \"payload\": {
                    \"source_path\": \"$source_path\",
                    \"dest_path\": \"$dest_path\"
                }
            }" > /dev/null
        
        if [[ $? -eq 0 ]]; then
            log "${GREEN}✓ Task created for $dir_name${NC}"
        else
            log "${RED}✗ Failed to create task for $dir_name${NC}"
        fi
        
    done < "$directories_file"
}

# Function to install dependencies
install_deps() {
    log "${BLUE}Installing Python dependencies...${NC}"
    pip3 install aiohttp psutil
    log "${GREEN}Dependencies installed${NC}"
}

# Main command handling
case "${1:-start}" in
    "start")
        log "${GREEN}Starting A2A Backup System...${NC}"
        
        # Install dependencies if needed
        if ! python3 -c "import aiohttp, psutil" 2>/dev/null; then
            install_deps
        fi
        
        start_coordinator
        if [[ $? -eq 0 ]]; then
            start_agents
            sleep 2
            show_status
        fi
        ;;
    
    "stop")
        stop_all
        ;;
    
    "restart")
        stop_all
        sleep 2
        $0 start
        ;;
    
    "status")
        show_status
        ;;
    
    "backup")
        create_backup_tasks
        ;;
    
    "install")
        install_deps
        ;;
    
    "logs")
        log "${BLUE}Recent logs:${NC}"
        if [[ -n "$2" ]]; then
            # Show specific log
            local log_file="$LOG_DIR/$2.log"
            if [[ -f "$log_file" ]]; then
                tail -n 20 "$log_file"
            else
                log "${RED}Log file not found: $log_file${NC}"
            fi
        else
            # Show all recent logs
            echo "=== Coordinator ==="
            tail -n 10 "$LOG_DIR/coordinator.log" 2>/dev/null || echo "No coordinator logs"
            echo
            echo "=== Agents ==="
            for i in $(seq 1 $NUM_AGENTS); do
                echo "--- Agent $i ---"
                tail -n 5 "$LOG_DIR/agent_$i.log" 2>/dev/null || echo "No agent $i logs"
            done
        fi
        ;;
    
    "help"|"-h"|"--help")
        echo "A2A Backup System Control Script"
        echo
        echo "Usage: $0 [command]"
        echo
        echo "Commands:"
        echo "  start       Start coordinator and agents (default)"
        echo "  stop        Stop all processes"
        echo "  restart     Stop and start again"
        echo "  status      Show system status"
        echo "  backup      Create backup tasks from backup_directories.txt"
        echo "  install     Install Python dependencies"
        echo "  logs [name] Show recent logs (optionally for specific component)"
        echo "  help        Show this help"
        echo
        echo "Environment variables:"
        echo "  NUM_AGENTS  Number of backup agents to start (default: 3)"
        echo
        echo "Examples:"
        echo "  $0 start                    # Start with 3 agents"
        echo "  NUM_AGENTS=5 $0 start       # Start with 5 agents"
        echo "  $0 logs coordinator         # Show coordinator logs"
        echo "  $0 backup                   # Create backup tasks"
        ;;
    
    *)
        log "${RED}Unknown command: $1${NC}"
        log "Use '$0 help' for usage information"
        exit 1
        ;;
esac