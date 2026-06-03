import { useState } from 'react';
import { useRecordingState } from './hooks/useRecordingState';
import SettingsPage from './components/settings/SettingsPage';
import HistoryPage from './components/history/HistoryPage';

type Page = 'settings' | 'history';

export default function App() {
  const [page, setPage] = useState<Page>('settings');
  const { state, lastError } = useRecordingState();

  const stateColor = {
    idle: 'bg-gray-400',
    recording: 'bg-recording animate-pulse',
    transcribing: 'bg-transcribing animate-pulse',
  }[state];

  return (
    <div className="flex flex-col h-screen bg-white text-gray-900 font-sans">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-200 select-none">
        <div className="flex items-center gap-2">
          <div className={`w-3 h-3 rounded-full ${stateColor}`} title={state} />
          <span className="font-semibold text-sm">SuperParler</span>
        </div>
        <nav className="flex gap-1">
          {(['settings', 'history'] as Page[]).map(p => (
            <button
              key={p}
              onClick={() => setPage(p)}
              className={`px-3 py-1 text-sm rounded-md transition-colors ${
                page === p
                  ? 'bg-brand-500 text-white'
                  : 'text-gray-600 hover:bg-gray-100'
              }`}
            >
              {p === 'settings' ? 'Réglages' : 'Historique'}
            </button>
          ))}
        </nav>
      </header>

      {/* Error banner */}
      {lastError && (
        <div className="bg-red-50 border-b border-red-200 px-4 py-2 text-sm text-red-700">
          {lastError}
        </div>
      )}

      {/* Content */}
      <main className="flex-1 overflow-auto">
        {page === 'settings' ? <SettingsPage /> : <HistoryPage />}
      </main>
    </div>
  );
}
