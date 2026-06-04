import { useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  onRecordingError,
  onRecordingResult,
  onRecordingStarted,
  onRecordingStopped,
  onRecordingTranscribing,
} from '../../lib/events';

type HudState = 'hidden' | 'recording' | 'transcribing' | 'success' | 'error';

// ── Sub-components ────────────────────────────────────────────────────────────

function Timer({ active }: { active: boolean }) {
  const [sec, setSec] = useState(0);
  useEffect(() => {
    if (!active) { setSec(0); return; }
    const id = setInterval(() => setSec(s => s + 1), 1000);
    return () => clearInterval(id);
  }, [active]);
  const mm = String(Math.floor(sec / 60)).padStart(2, '0');
  const ss = String(sec % 60).padStart(2, '0');
  return <span className="font-mono text-sm tabular-nums text-white/60">{mm}:{ss}</span>;
}

function Waveform() {
  // 14 bars with staggered durations and delays — no random values so React
  // renders consistently across mounts.
  const bars = Array.from({ length: 14 }, (_, i) => ({
    height: 6 + (i % 5) * 4,
    dur:    `${0.5 + (i % 4) * 0.1}s`,
    delay:  `${(i * 0.04).toFixed(2)}s`,
  }));
  return (
    <div className="flex items-end gap-px h-7">
      {bars.map(({ height, dur, delay }, i) => (
        <span
          key={i}
          className="inline-block w-0.5 rounded-full origin-bottom bg-white/70"
          style={{
            height: `${height}px`,
            animation: `wave-bar ${dur} ease-in-out infinite alternate`,
            animationDelay: delay,
          }}
        />
      ))}
    </div>
  );
}

function Spinner() {
  return (
    <svg className="animate-spin h-4 w-4 text-transcribing shrink-0" fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
    </svg>
  );
}

// ── HUD ───────────────────────────────────────────────────────────────────────

export function Hud() {
  const [hudState, setHudState] = useState<HudState>('hidden');
  const [errorMsg, setErrorMsg] = useState('');
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Make the overlay window fully click-through — the user should never feel it.
    const win = getCurrentWindow();
    void win.setIgnoreCursorEvents(true);

    function scheduleHide(delay: number) {
      if (hideTimer.current) clearTimeout(hideTimer.current);
      hideTimer.current = setTimeout(() => setHudState('hidden'), delay);
    }

    const unsubs = [
      onRecordingStarted(() => {
        if (hideTimer.current) clearTimeout(hideTimer.current);
        setHudState('recording');
      }),
      onRecordingStopped(() => setHudState('transcribing')),
      onRecordingTranscribing(() => setHudState('transcribing')),
      onRecordingResult(text => {
        if (text) { setHudState('success'); scheduleHide(2000); }
        else       setHudState('hidden');
      }),
      onRecordingError(msg => {
        setErrorMsg(msg);
        setHudState('error');
        scheduleHide(3000);
      }),
    ];

    return () => {
      if (hideTimer.current) clearTimeout(hideTimer.current);
      unsubs.forEach(p => void p.then(fn => fn()));
    };
  }, []);

  const visible = hudState !== 'hidden';

  return (
    // Full window area — transparent & pointer-events-none when idle.
    <div className="w-full h-full flex items-center justify-center p-3 pointer-events-none">
      <div
        className={`
          w-full rounded-2xl px-5 py-3.5
          bg-gray-900/88 backdrop-blur-md
          border border-white/10 shadow-2xl
          text-white
          transition-all duration-200 ease-out
          ${visible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-1.5 pointer-events-none'}
        `}
      >

        {hudState === 'recording' && (
          <div className="flex items-center gap-3 min-w-0">
            <span className="text-recording text-base leading-none animate-pulse shrink-0">●</span>
            <span className="text-sm font-semibold flex-1">Enregistrement</span>
            <Timer active />
            <Waveform />
          </div>
        )}

        {hudState === 'transcribing' && (
          <div className="flex items-center gap-3">
            <Spinner />
            <span className="text-sm font-semibold">Transcription en cours…</span>
          </div>
        )}

        {hudState === 'success' && (
          <div className="flex items-center gap-3">
            <span className="text-green-400 text-base leading-none shrink-0">✓</span>
            <span className="text-sm font-semibold">Texte collé !</span>
          </div>
        )}

        {hudState === 'error' && (
          <div className="flex items-center gap-2 min-w-0">
            <span className="text-red-400 text-base leading-none shrink-0">⚠</span>
            <span className="text-sm font-semibold truncate">{errorMsg || 'Erreur'}</span>
          </div>
        )}

      </div>
    </div>
  );
}
