// Overlay panel controller. Bundled by Vite as a separate entry (overlay.html),
// so `@tauri-apps/api` is properly imported — no `withGlobalTauri` needed.
//
// The Rust side (src-tauri/src/overlay.rs) emits `overlay:state` with one of:
//   "recording" | "transcribing" | "hide"
// The window is created lazily on the first dictation and reused afterwards.
import { listen } from '@tauri-apps/api/event';

type OverlayState = 'recording' | 'transcribing' | 'hide';

const BAR_COUNT = 48;
const panel = document.getElementById('panel');
const label = document.getElementById('label');
const wave = document.getElementById('wave');

// Build the waveform bars once. Each bar gets a peak height following an
// undulating envelope (taller mid-sections, tapered edges) plus a staggered
// animation delay, so the row shimmers like a live waveform — the SuperWhisper
// look — without needing real mic levels.
if (wave) {
  for (let i = 0; i < BAR_COUNT; i++) {
    const t = i / (BAR_COUNT - 1);
    const taper = Math.sin(t * Math.PI); // 0 → 1 → 0 across the row
    const undulation = 0.4 + 0.6 * Math.abs(Math.sin(t * Math.PI * 2.3));
    const peak = 4 + 30 * taper * undulation;
    const bar = document.createElement('span');
    bar.style.setProperty('--max', `${peak.toFixed(1)}px`);
    bar.style.setProperty('--d', `${i * 32}ms`);
    wave.appendChild(bar);
  }
}

function applyState(state: OverlayState): void {
  if (!panel || !label) return;

  if (state === 'hide') {
    panel.classList.remove('show');
    return;
  }

  panel.classList.remove('recording', 'transcribing');
  panel.classList.add(state, 'show');
  label.textContent =
    state === 'transcribing' ? 'Transcription…' : 'Enregistrement…';
}

// The first dictation is always "recording": fade in immediately on load so the
// panel is visible even if the initial event fires before this listener attaches.
applyState('recording');

void listen<OverlayState>('overlay:state', (event) => {
  applyState(event.payload);
});
