import { useTranscriptions } from '../../hooks/useTranscriptions';
import { Button } from '../shared/Button';
import { Spinner } from '../shared/Spinner';
import { TranscriptionList } from './TranscriptionList';

export default function HistoryPage() {
  const { transcriptions, loading, error, reload, remove, clear } = useTranscriptions();

  if (loading && transcriptions.length === 0) {
    return <div className="flex items-center justify-center h-full"><Spinner size="lg" /></div>;
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100">
        <span className="text-sm text-gray-500">{transcriptions.length} transcription(s)</span>
        <div className="flex gap-2">
          <Button size="sm" variant="secondary" onClick={() => reload()}>Rafraîchir</Button>
          {transcriptions.length > 0 && (
            <Button size="sm" variant="danger" onClick={clear}>Tout effacer</Button>
          )}
        </div>
      </div>
      {error && <div className="px-4 py-2 text-sm text-red-600">{error}</div>}
      <div className="flex-1 overflow-auto">
        {transcriptions.length === 0 ? (
          <div className="flex items-center justify-center h-full text-sm text-gray-400">
            Aucune transcription pour l'instant
          </div>
        ) : (
          <TranscriptionList transcriptions={transcriptions} onDelete={remove} />
        )}
      </div>
    </div>
  );
}
