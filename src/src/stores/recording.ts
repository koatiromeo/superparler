import { create } from 'zustand';
import type { RecordingState } from '../types';

interface RecordingStore {
  state: RecordingState;
  lastText: string | null;
  lastError: string | null;
  setState: (s: RecordingState) => void;
  setLastText: (text: string) => void;
  setLastError: (msg: string) => void;
  clearError: () => void;
}

export const useRecordingStore = create<RecordingStore>((set) => ({
  state: 'idle',
  lastText: null,
  lastError: null,
  setState: (state) => set({ state }),
  setLastText: (lastText) => set({ lastText, lastError: null }),
  setLastError: (lastError) => set({ lastError }),
  clearError: () => set({ lastError: null }),
}));
