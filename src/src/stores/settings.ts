import { create } from 'zustand';
import type { AppConfig, PartialConfig } from '../types';
import { getSettings, updateSettings } from '../lib/tauri';

interface SettingsStore {
  config: AppConfig | null;
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  update: (patch: PartialConfig) => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  config: null,
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const config = await getSettings();
      set({ config, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  update: async (patch: PartialConfig) => {
    set({ loading: true, error: null });
    try {
      const config = await updateSettings(patch);
      set({ config, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
}));
