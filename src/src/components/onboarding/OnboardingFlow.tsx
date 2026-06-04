import { useState } from 'react';
import { useSettingsStore } from '../../stores/settings';
import { HotkeyCapture } from '../settings/HotkeyCapture';
import { EngineSelector } from '../settings/EngineSelector';
import { GroqKeyField } from '../settings/GroqKeyField';
import { Button } from '../shared/Button';
import { Toggle } from '../shared/Toggle';
import type { Engine, RecordingMode } from '../../types';

interface Props {
  onComplete: () => void;
}

const IS_MAC = /Mac/i.test(navigator.platform);
const TOTAL = 4;

function Progress({ step }: { step: number }) {
  return (
    <div className="flex gap-1.5 justify-center mt-6">
      {Array.from({ length: TOTAL }, (_, i) => (
        <div
          key={i}
          className={`h-1.5 rounded-full transition-all duration-300 ${
            i < step ? 'w-6 bg-brand-500' : i === step ? 'w-6 bg-brand-500' : 'w-3 bg-gray-200'
          }`}
        />
      ))}
    </div>
  );
}

// ── Step 1: Bienvenue ─────────────────────────────────────────────────────────
function StepWelcome({ onNext }: { onNext: () => void }) {
  return (
    <div className="flex flex-col items-center text-center gap-6 py-4">
      <div className="text-6xl">🎙️</div>
      <div>
        <h1 className="text-2xl font-bold text-gray-900">Bienvenue dans SuperParler</h1>
        <p className="mt-2 text-sm text-gray-500 max-w-xs">
          Dictez n'importe où d'un simple raccourci clavier. Votre voix se transforme en texte
          et se colle directement dans l'application active.
        </p>
      </div>
      <ul className="text-sm text-gray-600 space-y-2 text-left">
        {['Offline-first — aucune donnée ne quitte votre machine',
          'Rapide — injecté en moins de 2 secondes avec Groq',
          'Discret — visible uniquement quand vous dictez'].map(f => (
          <li key={f} className="flex items-start gap-2">
            <span className="text-brand-500 shrink-0">✓</span>
            {f}
          </li>
        ))}
      </ul>
      <Button onClick={onNext} className="w-full max-w-xs">Commencer →</Button>
    </div>
  );
}

// ── Step 2: Permissions ───────────────────────────────────────────────────────
function StepPermissions({ onNext }: { onNext: () => void }) {
  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-lg font-bold text-gray-900">Permissions requises</h2>
        <p className="mt-1 text-sm text-gray-500">
          SuperParler injecte du texte en simulant Ctrl+V — une seule permission suffit.
        </p>
      </div>

      {IS_MAC ? (
        <div className="rounded-xl border border-amber-200 bg-amber-50 p-4 space-y-3">
          <p className="text-sm font-semibold text-amber-800">macOS — Accessibilité requise</p>
          <p className="text-xs text-amber-700">
            Pour injecter du texte dans d'autres applications, SuperParler a besoin de
            l'autorisation Accessibilité.
          </p>
          <ol className="text-xs text-amber-700 list-decimal list-inside space-y-1">
            <li>Réglages Système → Confidentialité et sécurité → Accessibilité</li>
            <li>Activez SuperParler dans la liste</li>
          </ol>
        </div>
      ) : (
        <div className="rounded-xl border border-green-200 bg-green-50 p-4">
          <p className="text-sm font-semibold text-green-800">Windows / Linux</p>
          <p className="text-xs text-green-700 mt-1">
            Aucune permission supplémentaire requise — SuperParler fonctionne directement.
          </p>
        </div>
      )}

      <Button onClick={onNext} className="w-full">
        {IS_MAC ? 'J\'ai activé l\'accessibilité →' : 'Continuer →'}
      </Button>
    </div>
  );
}

// ── Step 3: Moteur ────────────────────────────────────────────────────────────
function StepEngine({ onNext }: { onNext: () => void }) {
  const { config, update } = useSettingsStore();
  const engine: Engine = config?.engine ?? 'groq';

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-lg font-bold text-gray-900">Choisissez votre moteur STT</h2>
        <p className="mt-1 text-sm text-gray-500">
          Groq est idéal pour démarrer — rapide et précis, clé API gratuite.
        </p>
      </div>

      <EngineSelector value={engine} onChange={e => void update({ engine: e })} />

      {engine === 'groq' && <GroqKeyField />}

      {engine === 'local' && (
        <p className="text-xs text-amber-600">
          Le moteur local nécessite LLVM et <code>make models</code> pour télécharger
          ggml-small.bin (~500 MB). Vous pouvez configurer cela plus tard.
        </p>
      )}

      <Button onClick={onNext} className="w-full">Continuer →</Button>
    </div>
  );
}

// ── Step 4: Raccourci ─────────────────────────────────────────────────────────
function StepHotkey({ onComplete }: { onComplete: () => void }) {
  const { config, update } = useSettingsStore();
  const hotkey = config?.hotkey ?? 'CmdOrCtrl+Shift+Space';
  const mode: RecordingMode = config?.mode ?? 'pushToTalk';

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-lg font-bold text-gray-900">Configurez votre raccourci</h2>
        <p className="mt-1 text-sm text-gray-500">
          Ce raccourci global déclenche la dictée depuis n'importe quelle application.
        </p>
      </div>

      <HotkeyCapture value={hotkey} onChange={h => void update({ hotkey: h })} />

      <div className="space-y-2">
        <p className="text-sm font-medium text-gray-700">Mode d'activation</p>
        <Toggle
          checked={mode === 'pushToTalk'}
          onChange={v => void update({ mode: v ? 'pushToTalk' : 'toggle' })}
          label="Push-to-Talk (maintenir pour dicter)"
        />
        <p className="text-xs text-gray-400 pl-14">
          Sinon : Toggle (1er appui pour démarrer, 2e pour envoyer)
        </p>
      </div>

      <Button onClick={onComplete} className="w-full">C'est parti ! 🚀</Button>
    </div>
  );
}

// ── Main flow ─────────────────────────────────────────────────────────────────
export function OnboardingFlow({ onComplete }: Props) {
  const [step, setStep] = useState(0);
  const next = () => setStep(s => s + 1);

  // Pre-load settings so engine/hotkey steps can update config
  const { load, config } = useSettingsStore();
  if (!config) { void load(); }

  return (
    <div className="flex flex-col h-screen bg-white font-sans">
      <div className="flex-1 overflow-auto flex items-center justify-center p-6">
        <div className="w-full max-w-sm">
          {step === 0 && <StepWelcome onNext={next} />}
          {step === 1 && <StepPermissions onNext={next} />}
          {step === 2 && <StepEngine onNext={next} />}
          {step === 3 && <StepHotkey onComplete={onComplete} />}
        </div>
      </div>
      <Progress step={step} />
      <div className="h-6" />
    </div>
  );
}
