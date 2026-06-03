const LANGUAGES = [
  { code: 'fr', label: 'Français' },
  { code: 'en', label: 'English' },
  { code: 'es', label: 'Español' },
  { code: 'de', label: 'Deutsch' },
  { code: 'auto', label: 'Détection automatique' },
];

interface Props { value: string; onChange: (lang: string) => void; }

export function LanguageSelect({ value, onChange }: Props) {
  return (
    <label className="block text-sm text-gray-600">
      Langue de transcription
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        className="mt-1 block w-full rounded border border-gray-300 px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 bg-white"
      >
        {LANGUAGES.map(l => (
          <option key={l.code} value={l.code}>{l.label}</option>
        ))}
      </select>
    </label>
  );
}
