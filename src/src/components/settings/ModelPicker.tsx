import { useEffect, useState } from 'react';
import { listLocalModels } from '../../lib/tauri';

interface Props { value: string; onChange: (path: string) => void; }

export function ModelPicker({ value, onChange }: Props) {
  const [models, setModels] = useState<string[]>([]);

  useEffect(() => {
    listLocalModels().then(setModels).catch(() => setModels([]));
  }, []);

  return (
    <div className="mt-3">
      <label className="block text-sm text-gray-600 mb-1">Modèle local (GGUF)</label>
      {models.length > 0 ? (
        <select
          value={value}
          onChange={e => onChange(e.target.value)}
          className="block w-full rounded border border-gray-300 px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 bg-white"
        >
          {models.map(m => (
            <option key={m} value={m}>{m.split(/[/\\]/).pop()}</option>
          ))}
        </select>
      ) : (
        <p className="text-xs text-amber-600">
          Aucun modèle trouvé. Exécutez <code className="font-mono">make models</code> pour télécharger ggml-small.bin.
        </p>
      )}
    </div>
  );
}
