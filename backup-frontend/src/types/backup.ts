// Type definitions for the backup system

export interface DirectoryInfo {
  name: string;
  path: string;
  size: number;
  status: 'pending' | 'active' | 'completed' | 'error' | 'skipped' | 'in_progress';
  progress: number;
  selected: boolean;
  filesProcessed?: number;
  bytesProcessed?: number;
  currentFile?: string;
}

export interface MountStatus {
  mounted: boolean;
  path: string;
  message?: string;
}

export interface BackupStatus {
  state: 'stopped' | 'running' | 'paused';
  directories: DirectoryInfo[];
  totalSize: number;
  completedSize: number;
  startTime?: number;
  lastCompletedDir?: string;
  nextDir?: string;
  logs: LogEntry[];
  history: HistoryEntry[];
  localDiskSpace?: DiskSpace;
  remoteDiskSpace?: DiskSpace;
  mountStatus?: MountStatus;
}

export interface LogEntry {
  timestamp: string;
  level: 'info' | 'warning' | 'error';
  message: string;
}

export interface SpeedEntry {
  timestamp: number;
  speed: number;
  speedStr: string;
}

export interface BackupProfile {
  name: string;
  directories: string[];
}

export interface HistoryEntry {
  id: string;
  timestamp: string;
  duration: number;
  size: number;
  filesCount: number;
  status: 'completed' | 'failed' | 'cancelled';
}

export interface DiskSpace {
  free: number;
  total: number;
  used: number;
  percentage: number;
}

export interface ControlAction {
  action: 'start' | 'pause' | 'stop';
  parallel?: boolean;
}

export interface ScheduleConfig {
  type: 'daily' | 'weekly' | 'monthly' | 'custom';
  time: string;
  profile: string;
  enabled: boolean;
  lastRun?: string;
  nextRun?: string;
}