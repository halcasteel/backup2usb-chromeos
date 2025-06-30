import { create } from 'zustand';
import { BackupStatus } from '../types/backup';

interface BackupStore {
  status: BackupStatus | null;
  setStatus: (status: BackupStatus | null) => void;
  updateStatus: (updates: Partial<BackupStatus>) => void;
}

export const useBackupStore = create<BackupStore>((set) => ({
  status: null,
  setStatus: (status) => set({ status }),
  updateStatus: (updates) => set((state) => ({
    status: state.status ? { ...state.status, ...updates } : null,
  })),
}));