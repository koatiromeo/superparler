import { useState } from 'react';
import { setGroqKey } from '../../lib/tauri';
import { Button } from '../shared/Button';

export function GroqKeyField() {
  const [key, setKey] = useState('');
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    if (!key.trim()) return;
    setError(null);
    try {
      await setGroqKey(key.trim());
      setSaved(true);
      setKey('');
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="mt-3">
      <label className="block text-sm text-gray-600 mb-1">Clé API Groq</label>
      <div className="flex gap-2">
        <input
          type="password"
          value={key}
          onChange={e => setKey(e.target.value)}
          placeholder="gsk_..."
          className="flex-1 rounded border border-gray-300 px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
        />
        <Button size="sm" onClick={save} disabled={!key.trim()}>
          {saved ? '✓ Sauvegardé' : 'Enregistrer'}
        </Button>
      </div>
      {error && <p className="mt-1 text-xs text-red-600">{error}</p>}
      <p className="mt-1 text-xs text-gray-400">Stockée dans le trousseau OS, jamais en clair.</p>
    </div>
  );
}
