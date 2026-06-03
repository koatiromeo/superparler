// TODO: implement interactive hotkey capture (keyboard event listener → format as Tauri accelerator string)
// For now: simple text input for hotkey string
interface Props {
  value: string;
  onChange: (hotkey: string) => void;
}

export function HotkeyCapture({ value, onChange }: Props) {
  return (
    <label className="block text-sm text-gray-600">
      Raccourci global
      <input
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder="CmdOrCtrl+Shift+Space"
        className="mt-1 block w-full rounded border border-gray-300 px-3 py-1.5 text-sm font-mono focus:outline-none focus:ring-1 focus:ring-brand-500"
      />
      <span className="text-xs text-gray-400">Format : CmdOrCtrl+Shift+Space</span>
    </label>
  );
}
