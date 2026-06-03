import { useCallback, useEffect, useState } from 'react';
import { clearHistory, deleteTranscription, listTranscriptions } from '../lib/tauri';
import type { Transcription } from '../types';

export function useTranscriptions(limit = 50) {
  const [transcriptions, setTranscriptions] = useState<Transcription[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (offset = 0) => {
    setLoading(true);
    setError(null);
    try {
      const items = await listTranscriptions(limit, offset);
      setTranscriptions(items);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [limit]);

  useEffect(() => { load(); }, [load]);

  const remove = useCallback(async (id: string) => {
    await deleteTranscription(id);
    setTranscriptions(prev => prev.filter(t => t.id !== id));
  }, []);

  const clear = useCallback(async () => {
    await clearHistory();
    setTranscriptions([]);
  }, []);

  return { transcriptions, loading, error, reload: load, remove, clear };
}
