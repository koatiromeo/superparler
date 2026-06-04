import { useEffect } from 'react';
import { onRecordingResult } from '../../lib/events';
import { useTranscriptions } from '../../hooks/useTranscriptions';
import { Button } from '../shared/Button';
import { Spinner } from '../shared/Spinner';
import { TranscriptionList } from './TranscriptionList';

export default function HistoryPage() {
  const { transcriptions, loading, error, reload, remove, clear } = useTranscriptions();

  // Reload when a new transcription arrives from the backend
  useEffect(() => {
    const unlisten = onRecordingResult(text => {
      if (text) void reload();
    });
    return () => { void unlisten.then(fn => fn()); };
  }, [reload]);

  if (loading && transcriptions.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spinner size="lg" />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-gray-100 bg-gray-50/50">
        <span className="text-xs font-medium text-gray-500">
          {transcriptions.length
            ? `${transcriptions.length} dictée${transcriptions.length > 1 ? 's' : ''}`
            : 'Historique'}
        </span>
        <div className="flex gap-2">
          <Button size="sm" variant="secondary" onClick={() => void reload()}>
            Rafraîchir
          </Button>
          {transcriptions.length > 0 && (
            <Button size="sm" variant="danger" onClick={() => void clear()}>
              Tout effacer
            </Button>
          )}
        </div>
      </div>

      {error && (
        <div className="px-4 py-2 text-sm text-red-600 bg-red-50 border-b border-red-100">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-auto">
        {transcriptions.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-center px-8">
            <span className="text-3xl">🎙️</span>
            <p className="text-sm font-medium text-gray-600">Aucune dictée pour l'instant</p>
            <p className="text-xs text-gray-400">
              Appuyez sur votre raccourci global pour dicter du texte — il apparaîtra ici.
            </p>
          </div>
        ) : (
          <TranscriptionList transcriptions={transcriptions} onDelete={remove} />
        )}
      </div>
    </div>
  );
}
