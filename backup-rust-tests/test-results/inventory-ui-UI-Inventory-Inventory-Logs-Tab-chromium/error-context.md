# Page snapshot

```yaml
- heading "Backup Operations Dashboard" [level=2]
- text: PAUSED
- paragraph: SOURCE DISK
- paragraph: 929.45 MB
- paragraph: (98.5% used)
- paragraph: BACKUP DISK
- paragraph: 1.86 TB
- paragraph: (0.0% used)
- group:
  - term: Total Directories
  - definition: "100"
  - term: Completed
  - definition: "0"
  - term: Total Size
  - definition: 50.88 KB
  - term: Speed
  - definition: —
- tablist:
  - tab "Backup"
  - tab "Logs" [selected]
  - tab "Schedule"
  - tab "History"
- tabpanel "Logs":
  - paragraph: RSYNC LOGS
  - group:
    - button "All"
    - button "Errors"
    - button "Warnings"
    - button "Info"
  - button "Download Log File"
  - img
  - textbox "Search logs..."
  - paragraph: No logs to display
- button "START"
- button "PAUSE" [disabled]
- button "STOP"
- region "Notifications-top"
- region "Notifications-top-left"
- region "Notifications-top-right"
- region "Notifications-bottom-left"
- region "Notifications-bottom"
- region "Notifications-bottom-right"
```