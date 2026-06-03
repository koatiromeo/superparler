import type { Transcription } from '../../types';
import { TranscriptionItem } from './TranscriptionItem';

interface Props {
  transcriptions: Transcription[];
  onDelete: (id: string) => void;
}

export function TranscriptionList({ transcriptions, onDelete }: Props) {
  return (
    <ul className="divide-y divide-gray-100">
      {transcriptions.map(t => (
        <TranscriptionItem key={t.id} transcription={t} onDelete={() => onDelete(t.id)} />
      ))}
    </ul>
  );
}
