import { useEffect } from 'react';
import {
  onRecordingError,
  onRecordingResult,
  onRecordingStarted,
  onRecordingStopped,
  onRecordingTranscribing,
} from '../lib/events';
import { useRecordingStore } from '../stores/recording';

export function useRecordingState() {
  const { state, lastText, lastError, setState, setLastText, setLastError } = useRecordingStore();

  useEffect(() => {
    const unlisteners = [
      onRecordingStarted(() => setState('recording')),
      onRecordingStopped(() => setState('transcribing')),
      onRecordingTranscribing(() => setState('transcribing')),
      onRecordingResult((text) => {
        setState('idle');
        setLastText(text);
      }),
      onRecordingError((msg) => {
        setState('idle');
        setLastError(msg);
      }),
    ];

    return () => {
      unlisteners.forEach(p => p.then(fn => fn()));
    };
  }, [setState, setLastText, setLastError]);

  return { state, lastText, lastError };
}
