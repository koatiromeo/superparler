import { useCallback, useEffect, useState } from 'react';

interface Props {
  value: string;
  onChange: (hotkey: string) => void;
}

const IS_MAC = /Mac/i.test(navigator.platform);

/** Convert a KeyboardEvent to a Tauri accelerator string, or null if not usable. */
function toAccelerator(e: KeyboardEvent): string | null {
  if (['Control', 'Meta', 'Shift', 'Alt', 'CapsLock', 'Tab'].includes(e.key)) return null;

  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push('CmdOrCtrl');
  if (e.altKey)               mods.push('Alt');
  if (e.shiftKey)             mods.push('Shift');
  if (mods.length === 0) return null; // require at least one modifier

  let key: string;
  if      (e.code === 'Space')                        key = 'Space';
  else if (e.code.startsWith('Key'))                  key = e.code.slice(3);
  else if (e.code.startsWith('Digit'))                key = e.code.slice(5);
  else if (/^F\d+$/.test(e.code))                    key = e.code;
  else if (e.code === 'Backspace')                    key = 'Backspace';
  else if (e.code === 'Delete')                       key = 'Delete';
  else if (e.code === 'Return' || e.code === 'Enter') key = 'Return';
  else                                                key = e.key.toUpperCase();

  return [...mods, key].join('+');
}

/** Split a Tauri accelerator string into human-readable key labels. */
function splitKeycaps(hotkey: string): string[] {
  return hotkey.split('+').map(k => {
    if (k === 'CmdOrCtrl') return IS_MAC ? '⌘' : 'Ctrl';
    if (k === 'Shift')     return IS_MAC ? '⇧' : 'Shift';
    if (k === 'Alt')       return IS_MAC ? '⌥' : 'Alt';
    if (k === 'Space')     return '␣';
    if (k === 'Return')    return '↵';
    return k;
  });
}

function Keycap({ label }: { label: string }) {
  return (
    <kbd className="inline-flex items-center justify-center min-w-[2rem] h-7 px-2 rounded border border-gray-300 bg-gray-50 shadow-[0_1px_0_0_theme(colors.gray.300)] text-xs font-semibold font-mono text-gray-700 select-none">
      {label}
    </kbd>
  );
}

export function HotkeyCapture({ value, onChange }: Props) {
  const [capturing, setCapturing] = useState(false);

  const cancel = useCallback(() => setCapturing(false), []);

  useEffect(() => {
    if (!capturing) return;

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape') { cancel(); return; }
      const accel = toAccelerator(e);
      if (accel) { onChange(accel); setCapturing(false); }
    };

    window.addEventListener('keydown', onKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', onKeyDown, { capture: true });
  }, [capturing, onChange, cancel]);

  const caps = splitKeycaps(value);

  return (
    <div className="space-y-2">
      <span className="block text-sm font-medium text-gray-700">Raccourci global</span>

      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1">
          {caps.map((cap, i) => <Keycap key={i} label={cap} />)}
        </div>

        <button
          onClick={capturing ? cancel : () => setCapturing(true)}
          className={`px-3 py-1.5 text-xs rounded border font-medium transition-colors ${
            capturing
              ? 'border-red-300 bg-red-50 text-red-700 hover:bg-red-100'
              : 'border-gray-300 bg-white text-gray-600 hover:bg-gray-50'
          }`}
        >
          {capturing ? 'Annuler' : 'Modifier'}
        </button>
      </div>

      {capturing && (
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-brand-50 border border-brand-200 text-sm text-brand-700">
          <span className="animate-pulse">●</span>
          <span>Appuyez sur le raccourci… (Échap pour annuler)</span>
        </div>
      )}
    </div>
  );
}
