# Backup System Architecture Proposal

## Current Issues (Monolithic Design)
- Single 920+ line file with mixed responsibilities
- HTTP server, backup logic, file operations all coupled
- Hard to test individual components
- Difficult to extend or modify safely

## Proposed Structure (Modular Design)

```
backup_system/
├── core/
│   ├── __init__.py
│   ├── backup_manager.py      # Core backup orchestration
│   ├── directory_scanner.py   # Directory discovery & sizing
│   ├── rsync_wrapper.py       # Rsync process management
│   └── status_tracker.py      # State persistence & updates
├── server/
│   ├── __init__.py
│   ├── http_server.py         # HTTP server & routing
│   ├── api_handlers.py        # REST API endpoints
│   └── websocket_handler.py   # Real-time updates (future)
├── workers/
│   ├── __init__.py
│   ├── single_process.py      # Sequential backup worker
│   ├── multi_process.py       # Parallel backup worker
│   └── scheduler.py           # Scheduled backup management
├── config/
│   ├── __init__.py
│   ├── settings.py            # Configuration management
│   └── profiles.py            # Backup profiles
├── utils/
│   ├── __init__.py
│   ├── file_utils.py          # File operations
│   ├── size_utils.py          # Size calculation & formatting
│   └── logging_config.py      # Logging setup
├── web/
│   ├── static/
│   │   ├── css/
│   │   ├── js/
│   │   └── assets/
│   └── templates/
│       └── dashboard.html
├── tests/
│   ├── test_backup_manager.py
│   ├── test_api_handlers.py
│   └── test_workers.py
└── main.py                    # Application entry point
```

## Benefits of Modular Design
1. **Single Responsibility**: Each module has one clear purpose
2. **Testability**: Can unit test individual components
3. **Maintainability**: Easier to modify specific functionality
4. **Scalability**: Can add new backup strategies/workers
5. **Reusability**: Components can be used in different contexts
6. **Deployment**: Better containerization & distribution

## Implementation Priority
1. **Phase 1**: Extract core backup logic
2. **Phase 2**: Separate HTTP server concerns  
3. **Phase 3**: Add worker abstraction layer
4. **Phase 4**: Enhanced configuration management
5. **Phase 5**: Real-time updates via WebSocket

## Multi-Process Architecture
```
Main Process (HTTP Server)
├── Worker Pool Manager
│   ├── Worker 1: /home/user/Documents/
│   ├── Worker 2: /home/user/Downloads/  
│   └── Worker 3: /home/user/Projects/
├── Status Aggregator
└── Progress Reporter (WebSocket)
```

This would allow true parallel processing while maintaining coordination.