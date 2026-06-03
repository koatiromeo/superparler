import { useSettings } from '../../hooks/useSettings';
import { Spinner } from '../shared/Spinner';
import { EngineSelector } from './EngineSelector';
import { HotkeyCapture } from './HotkeyCapture';
import { LanguageSelect } from './LanguageSelect';
import { GroqKeyField } from './GroqKeyField';
import { ModelPicker } from './ModelPicker';
import { Toggle } from '../shared/Toggle';

export default function SettingsPage() {
  const { config, loading, error, update } = useSettings();

  if (loading && !config) return (
    <div className="flex items-center justify-center h-full"><Spinner size="lg" /></div>
  );
  if (error) return (
    <div className="p-4 text-red-600 text-sm">Erreur de chargement : {error}</div>
  );
  if (!config) return null;

  return (
    <div className="p-6 space-y-6 max-w-lg">
      <section>
        <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">Moteur STT</h2>
        <EngineSelector value={config.engine} onChange={engine => update({ engine })} />
        {config.engine === 'local' && (
          <ModelPicker value={config.localModelPath} onChange={localModelPath => update({ localModelPath })} />
        )}
        {config.engine === 'groq' && (
          <>
            <GroqKeyField />
            <label className="block mt-3 text-sm text-gray-600">
              Modèle Groq
              <input
                type="text"
                value={config.groqModel}
                onChange={e => update({ groqModel: e.target.value })}
                className="mt-1 block w-full rounded border border-gray-300 px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
              />
            </label>
          </>
        )}
      </section>

      <section>
        <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">Capture</h2>
        <HotkeyCapture value={config.hotkey} onChange={hotkey => update({ hotkey })} />
        <div className="mt-3 flex gap-4">
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input type="radio" checked={config.mode === 'pushToTalk'} onChange={() => update({ mode: 'pushToTalk' })} />
            Push-to-Talk
          </label>
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input type="radio" checked={config.mode === 'toggle'} onChange={() => update({ mode: 'toggle' })} />
            Toggle
          </label>
        </div>
      </section>

      <section>
        <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">Langue & Injection</h2>
        <LanguageSelect value={config.language} onChange={language => update({ language })} />
        <div className="mt-3 flex gap-4">
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input type="radio" checked={config.injectMethod === 'paste'} onChange={() => update({ injectMethod: 'paste' })} />
            Coller (Ctrl/Cmd+V)
          </label>
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input type="radio" checked={config.injectMethod === 'type'} onChange={() => update({ injectMethod: 'type' })} />
            Saisie caractère par caractère
          </label>
        </div>
      </section>

      <section>
        <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">Amélioration (v2)</h2>
        <Toggle
          checked={config.enhanceEnabled}
          onChange={enhanceEnabled => update({ enhanceEnabled })}
          label="Activer le reformatage IA"
        />
        {config.enhanceEnabled && (
          <textarea
            value={config.enhancePrompt}
            onChange={e => update({ enhancePrompt: e.target.value })}
            placeholder="Ex : Corrige et formate en email professionnel"
            rows={3}
            className="mt-2 block w-full rounded border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 resize-none"
          />
        )}
      </section>

      <section>
        <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">Système</h2>
        <Toggle
          checked={config.launchAtStartup}
          onChange={launchAtStartup => update({ launchAtStartup })}
          label="Lancer au démarrage"
        />
      </section>
    </div>
  );
}
