import { useState } from 'react';
import { useSettingsStore } from '../../stores/settings';
import { HotkeyCapture } from '../settings/HotkeyCapture';
import { GroqKeyField } from '../settings/GroqKeyField';
import { Button } from '../shared/Button';
import { Toggle } from '../shared/Toggle';
import type { RecordingMode } from '../../types';

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
            i <= step ? 'w-6 bg-brand-500' : 'w-3 bg-gray-200'
          }`}
        />
      ))}
    </div>
  );
}

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
        {[
          'Rapide — injecté en moins de 2 secondes avec Groq',
          'Précis — whisper-large-v3-turbo',
          'Discret — visible uniquement dans la barre des tâches',
        ].map(f => (
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
        {IS_MAC ? "J'ai activé l'accessibilité →" : 'Continuer →'}
      </Button>
    </div>
  );
}

function StepEngine({ onNext }: { onNext: () => void }) {
  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-lg font-bold text-gray-900">Configurez Groq</h2>
        <p className="mt-1 text-sm text-gray-500">
          SuperParler utilise Groq pour transcrire votre voix — rapide, précis, clé API gratuite.
        </p>
      </div>

      <div className="rounded-xl border border-brand-200 bg-brand-50 px-4 py-3">
        <p className="text-sm font-semibold text-brand-700">⚡ Groq Whisper</p>
        <p className="text-xs text-brand-600 mt-0.5">
          Créez un compte sur <span className="font-mono">console.groq.com</span> → API Keys → Create key
        </p>
      </div>

      <GroqKeyField />

      <Button onClick={onNext} className="w-full">Continuer →</Button>
    </div>
  );
}

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

export function OnboardingFlow({ onComplete }: Props) {
  const [step, setStep] = useState(0);
  const next = () => setStep(s => s + 1);
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
