import { useState } from 'react';
import type { Transcription } from '../../types';
import { Button } from '../shared/Button';

interface Props {
  transcription: Transcription;
  onDelete: () => void;
}

export function TranscriptionItem({ transcription: t, onDelete }: Props) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(t.text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const date = new Date(t.createdAt).toLocaleString('fr-FR', {
    day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit',
  });
  const duration = (t.durationMs / 1000).toFixed(1);

  return (
    <li className="px-4 py-3 hover:bg-gray-50 group">
      <p className="text-sm text-gray-900 leading-relaxed">{t.text}</p>
      <div className="mt-1.5 flex items-center justify-between">
        <span className="text-xs text-gray-400 flex items-center gap-1.5 flex-wrap">
          {date} · {duration}s · {t.engine}
          {t.enhanced === 1 && (
            <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-xs font-medium bg-purple-100 text-purple-700">
              ✨ Reformulé
            </span>
          )}
        </span>
        <div className="flex gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity">
          <Button size="sm" variant="secondary" onClick={copy}>
            {copied ? 'Copié !' : 'Copier'}
          </Button>
          <Button size="sm" variant="danger" onClick={onDelete}>Supprimer</Button>
        </div>
      </div>
    </li>
  );
}
