# Backup System Endpoint Test Plan

## Prerequisites
1. Ensure server is running: `./target/release/backup-system`
2. Wait for initial directory scan to complete (check CPU usage drops)
3. Have terminal ready for curl commands

## Test Execution Plan

### 1. Basic Health Check
```bash
# Test: Server is responding
curl -v http://localhost:8888/
# Expected: 200 OK with HTML content (frontend)
```

### 2. API Status Endpoint
```bash
# Test: Get current backup status
curl -s http://localhost:8888/api/status | jq .
# Expected: JSON with status, directories, progress info
# Fields to verify:
# - status: "stopped"
# - directories: array of directory objects
# - progress: 0
# - totalSize/completedSize: numeric values
```

### 3. Static File Serving
```bash
# Test: Frontend assets are served
curl -I http://localhost:8888/index.html
curl -I http://localhost:8888/assets/index-DeQ8XGvy.js
# Expected: 200 OK with proper content-type headers
```

### 4. Start Backup
```bash
# Test: Start backup process
curl -X POST http://localhost:8888/start
# Expected: 200 OK
# Verify: Call /api/status again - status should be "running"
```

### 5. WebSocket Connection
```bash
# Test: WebSocket real-time updates
# Option 1: Using wscat (install: npm install -g wscat)
wscat -c ws://localhost:8888/ws
# Expected: Connection established, receive status updates

# Option 2: Using curl (newer versions)
curl --include \
     --no-buffer \
     --header "Connection: Upgrade" \
     --header "Upgrade: websocket" \
     --header "Sec-WebSocket-Key: SGVsbG8sIHdvcmxkIQ==" \
     --header "Sec-WebSocket-Version: 13" \
     http://localhost:8888/ws
```

### 6. Pause Backup
```bash
# Test: Pause running backup
curl -X POST http://localhost:8888/pause
# Expected: 200 OK
# Verify: /api/status shows status: "paused"
```

### 7. Resume Backup
```bash
# Test: Resume paused backup
curl -X POST http://localhost:8888/start
# Expected: 200 OK
# Verify: /api/status shows status: "running"
```

### 8. Stop Backup
```bash
# Test: Stop backup completely
curl -X POST http://localhost:8888/stop
# Expected: 200 OK
# Verify: /api/status shows status: "stopped"
```

### 9. Directory Selection (if implemented)
```bash
# Test: Select specific directories
curl -X POST http://localhost:8888/api/select \
  -H "Content-Type: application/json" \
  -d '{"directories": [0, 2, 5]}'
# Expected: 200 OK or 404 if not implemented
```

### 10. Profile Management (if implemented)
```bash
# Test: Get profiles
curl http://localhost:8888/api/profile
# Expected: JSON array of profiles or 404

# Test: Create profile
curl -X POST http://localhost:8888/api/profile \
  -H "Content-Type: application/json" \
  -d '{"name": "Development", "directories": ["src", "projects"]}'
# Expected: 200 OK or 404
```

### 11. Logs Endpoint (if implemented)
```bash
# Test: Get backup logs
curl "http://localhost:8888/api/logs?limit=50"
# Expected: JSON array of log entries or 404
```

### 12. History Endpoint (if implemented)
```bash
# Test: Get backup history
curl http://localhost:8888/api/history
# Expected: JSON array of previous backups or 404
```

### 13. Schedule Endpoint (if implemented)
```bash
# Test: Get schedules
curl http://localhost:8888/api/schedule
# Expected: JSON array of schedules or 404
```

## Error Testing

### 14. Invalid Endpoints
```bash
# Test: Non-existent endpoint
curl http://localhost:8888/api/invalid
# Expected: 404 Not Found
```

### 15. Method Not Allowed
```bash
# Test: Wrong HTTP method
curl -X GET http://localhost:8888/start
# Expected: 405 Method Not Allowed
```

### 16. Server Restart Recovery
```bash
# 1. Start a backup
curl -X POST http://localhost:8888/start

# 2. Kill the server
pkill backup-system

# 3. Restart the server
./target/release/backup-system &

# 4. Check status
curl http://localhost:8888/api/status | jq .
# Expected: Previous session restored in "paused" state
```

## Performance Testing

### 17. Concurrent Requests
```bash
# Test: Multiple status requests
for i in {1..10}; do
  curl -s http://localhost:8888/api/status &
done
wait
# Expected: All requests succeed without errors
```

### 18. Large Directory Handling
```bash
# Observe: Check memory and CPU usage during scan
# Watch: Response time of /api/status with many directories
```

## Validation Checklist

- [ ] Frontend loads at http://localhost:8888
- [ ] API status returns valid JSON
- [ ] Start/pause/stop commands work correctly
- [ ] Status transitions are correct (stopped->running->paused->stopped)
- [ ] WebSocket sends real-time updates
- [ ] Session persists across restarts
- [ ] No memory leaks during long operations
- [ ] Error responses are properly formatted
- [ ] CORS headers are present for API calls

## Common Issues to Watch For

1. **High CPU during scan**: Normal, should settle after initial scan
2. **Database locked errors**: Check only one instance is running
3. **WebSocket disconnects**: Check for timeout settings
4. **Missing directories**: Verify home directory permissions
5. **Frontend 404**: Ensure static files were copied correctly

## Test Results Log

```
Date: _______________
Tester: _____________

[ ] Health Check         Pass/Fail: ______ Notes: ________________
[ ] API Status          Pass/Fail: ______ Notes: ________________
[ ] Static Files        Pass/Fail: ______ Notes: ________________
[ ] Start Backup        Pass/Fail: ______ Notes: ________________
[ ] WebSocket           Pass/Fail: ______ Notes: ________________
[ ] Pause Backup        Pass/Fail: ______ Notes: ________________
[ ] Resume Backup       Pass/Fail: ______ Notes: ________________
[ ] Stop Backup         Pass/Fail: ______ Notes: ________________
[ ] Session Recovery    Pass/Fail: ______ Notes: ________________
```