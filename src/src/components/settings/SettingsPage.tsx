import { useState } from 'react';
import { useSettings } from '../../hooks/useSettings';
import { Spinner } from '../shared/Spinner';
import { HotkeyCapture } from './HotkeyCapture';
import { LanguageSelect } from './LanguageSelect';
import { GroqKeyField } from './GroqKeyField';
import { Toggle } from '../shared/Toggle';

type Section = 'general' | 'engine' | 'hotkey' | 'enhancement' | 'about';

const NAV: { id: Section; label: string; icon: string }[] = [
  { id: 'general',     label: 'Général',      icon: '⚙️' },
  { id: 'engine',      label: 'Moteur STT',   icon: '🎙️' },
  { id: 'hotkey',      label: 'Raccourci',    icon: '⌨️' },
  { id: 'enhancement', label: 'Reformulation', icon: '✨' },
  { id: 'about',       label: 'À propos',     icon: 'ℹ️' },
];

function H({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-widest mb-4">
      {children}
    </h2>
  );
}

function Divider() {
  return <hr className="border-gray-100 my-5" />;
}

function RadioRow({
  label, checked, onChange, description,
}: {
  label: string; checked: boolean; onChange: () => void; description?: string;
}) {
  return (
    <label className="flex items-start gap-3 cursor-pointer py-1 group">
      <input type="radio" checked={checked} onChange={onChange} className="mt-0.5 accent-brand-500" />
      <span>
        <span className="text-sm font-medium text-gray-800 group-hover:text-gray-900">{label}</span>
        {description && <span className="block text-xs text-gray-500 mt-0.5">{description}</span>}
      </span>
    </label>
  );
}

export default function SettingsPage() {
  const [section, setSection] = useState<Section>('general');
  const { config, loading, error, update } = useSettings();

  if (loading && !config) {
    return <div className="flex items-center justify-center h-full"><Spinner size="lg" /></div>;
  }
  if (error) return <div className="p-6 text-sm text-red-600">Erreur : {error}</div>;
  if (!config) return null;

  return (
    <div className="flex h-full">
      <nav className="w-44 shrink-0 border-r border-gray-200 bg-gray-50/70 py-2">
        {NAV.map(item => (
          <button
            key={item.id}
            onClick={() => setSection(item.id)}
            className={`w-full flex items-center gap-2.5 px-4 py-2.5 text-sm text-left transition-colors ${
              section === item.id
                ? 'bg-white font-semibold text-gray-900 border-r-2 border-brand-500 shadow-sm'
                : 'text-gray-600 hover:bg-gray-100 hover:text-gray-800'
            }`}
          >
            <span className="text-base leading-none">{item.icon}</span>
            {item.label}
          </button>
        ))}
      </nav>

      <div className="flex-1 overflow-auto px-7 py-6">

        {section === 'general' && (
          <div>
            <H>Langue & Injection</H>
            <LanguageSelect value={config.language} onChange={language => update({ language })} />
            <Divider />
            <p className="text-sm font-medium text-gray-700 mb-2">Méthode d'injection</p>
            <div className="space-y-1">
              <RadioRow
                label="Coller (Ctrl/Cmd+V)"
                description="Rapide — utilise le presse-papiers"
                checked={config.injectMethod === 'paste'}
                onChange={() => update({ injectMethod: 'paste' })}
              />
              <RadioRow
                label="Saisie caractère par caractère"
                description="Lent mais compatible avec tous les champs"
                checked={config.injectMethod === 'type'}
                onChange={() => update({ injectMethod: 'type' })}
              />
            </div>
            <Divider />
            <Toggle
              checked={config.launchAtStartup}
              onChange={launchAtStartup => update({ launchAtStartup })}
              label="Lancer SuperParler au démarrage"
            />
          </div>
        )}

        {section === 'engine' && (
          <div>
            <H>Moteur de transcription — Groq</H>

            <div className="rounded-xl border border-brand-200 bg-brand-50 px-4 py-3 mb-5">
              <p className="text-sm font-semibold text-brand-700">⚡ Groq Whisper (cloud)</p>
              <p className="text-xs text-brand-600 mt-0.5">whisper-large-v3-turbo · Rapide · Précis</p>
            </div>

            <div className="space-y-4">
              <GroqKeyField />

              <label className="block">
                <span className="text-sm font-medium text-gray-700">Modèle Groq</span>
                <input
                  type="text"
                  value={config.groqModel}
                  onChange={e => update({ groqModel: e.target.value })}
                  className="mt-1.5 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-transparent"
                />
                <span className="mt-1 block text-xs text-gray-400">
                  Ex : whisper-large-v3-turbo
                </span>
              </label>
            </div>
          </div>
        )}

        {section === 'hotkey' && (
          <div>
            <H>Raccourci global</H>
            <HotkeyCapture value={config.hotkey} onChange={hotkey => update({ hotkey })} />
            <Divider />
            <p className="text-sm font-medium text-gray-700 mb-2">Mode d'activation</p>
            <div className="space-y-1">
              <RadioRow
                label="Push-to-Talk"
                description="Maintenir le raccourci pour dicter, relâcher pour envoyer"
                checked={config.mode === 'pushToTalk'}
                onChange={() => update({ mode: 'pushToTalk' })}
              />
              <RadioRow
                label="Toggle"
                description="Premier appui pour démarrer, deuxième appui pour envoyer"
                checked={config.mode === 'toggle'}
                onChange={() => update({ mode: 'toggle' })}
              />
            </div>
          </div>
        )}

        {section === 'enhancement' && (
          <div>
            <H>Reformulation IA</H>
            <Toggle
              checked={config.enhanceEnabled}
              onChange={enhanceEnabled => update({ enhanceEnabled })}
              label="Activer le reformatage automatique"
            />
            {config.enhanceEnabled ? (
              <div className="mt-4 space-y-4">
                <label className="block">
                  <span className="text-sm font-medium text-gray-700">Instruction de reformulation</span>
                  <textarea
                    value={config.enhancePrompt}
                    onChange={e => update({ enhancePrompt: e.target.value })}
                    placeholder="Ex : Corrige l'orthographe et la ponctuation. Écris en style professionnel."
                    rows={4}
                    className="mt-1.5 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-transparent resize-none"
                  />
                </label>
                <label className="block">
                  <span className="text-sm font-medium text-gray-700">Modèle LLM</span>
                  <input
                    type="text"
                    value={config.enhanceModel}
                    onChange={e => update({ enhanceModel: e.target.value })}
                    className="mt-1.5 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-transparent"
                  />
                  <span className="mt-1 block text-xs text-gray-400">
                    Ex : llama-3.3-70b-versatile · llama-3.1-8b-instant
                  </span>
                </label>
              </div>
            ) : (
              <p className="mt-3 text-sm text-gray-500">
                Activez pour reformatter automatiquement le texte via Groq avant injection.
              </p>
            )}
          </div>
        )}

        {section === 'about' && (
          <div>
            <H>À propos</H>
            <div className="space-y-1">
              <p className="text-xl font-bold text-gray-900">SuperParler</p>
              <p className="text-sm text-gray-500">Version 0.1.0</p>
            </div>
            <p className="mt-4 text-sm text-gray-600 leading-relaxed">
              Dictée vocale rapide et précise via Groq Whisper, injectée directement au curseur
              dans n'importe quelle application.
            </p>
            <Divider />
            <dl className="space-y-2 text-sm">
              {[
                ['Interface', 'Tauri 2 · React 19 · TypeScript'],
                ['Backend',   'Rust 2024'],
                ['Audio',     'cpal · Silero VAD · rubato'],
                ['STT',       'Groq API (whisper-large-v3-turbo)'],
                ['Injection', 'enigo · clipboard'],
              ].map(([k, v]) => (
                <div key={k} className="flex gap-3">
                  <dt className="w-28 shrink-0 text-gray-400">{k}</dt>
                  <dd className="text-gray-700 font-mono text-xs">{v}</dd>
                </div>
              ))}
            </dl>
          </div>
        )}
      </div>
    </div>
  );
}
