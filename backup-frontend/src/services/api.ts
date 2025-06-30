import { BackupStatus } from '../types/backup';

const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8888';

export async function fetchStatus(): Promise<BackupStatus> {
  const response = await fetch(`${API_BASE_URL}/api/status`);
  if (!response.ok) {
    throw new Error(`Failed to fetch status: ${response.statusText}`);
  }
  return response.json();
}

export async function startBackup(): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/start`, {
    method: 'POST',
  });
  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    if (data.error) {
      throw new Error(data.error);
    }
    throw new Error(`Failed to start backup: ${response.statusText}`);
  }
}

export async function pauseBackup(): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/pause`, {
    method: 'POST',
  });
  if (!response.ok) {
    throw new Error(`Failed to pause backup: ${response.statusText}`);
  }
}

export async function stopBackup(): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/stop`, {
    method: 'POST',
  });
  if (!response.ok) {
    throw new Error(`Failed to stop backup: ${response.statusText}`);
  }
}