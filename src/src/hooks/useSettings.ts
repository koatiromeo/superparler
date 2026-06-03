import { useEffect } from 'react';
import { useSettingsStore } from '../stores/settings';

export function useSettings() {
  const { config, loading, error, load, update } = useSettingsStore();

  useEffect(() => {
    if (!config) {
      load();
    }
  }, [config, load]);

  return { config, loading, error, update };
}
