import type { Engine } from '../../types';

interface Props {
  value: Engine;
  onChange: (engine: Engine) => void;
}

export function EngineSelector({ value, onChange }: Props) {
  return (
    <div className="flex gap-3">
      {(['local', 'groq'] as Engine[]).map(engine => (
        <button
          key={engine}
          onClick={() => onChange(engine)}
          className={`flex-1 rounded-lg border-2 px-4 py-3 text-sm font-medium transition-colors ${
            value === engine
              ? 'border-brand-500 bg-brand-50 text-brand-700'
              : 'border-gray-200 text-gray-600 hover:border-gray-300'
          }`}
        >
          {engine === 'local' ? '🔒 Local (offline)' : '⚡ Groq (cloud)'}
          <p className="mt-1 text-xs font-normal text-gray-500">
            {engine === 'local' ? 'Whisper GGUF • Privé' : 'whisper-large-v3-turbo • Rapide'}
          </p>
        </button>
      ))}
    </div>
  );
}
