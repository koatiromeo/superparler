// Mirror of ALL Rust IPC structs — must stay in sync with src-tauri/src/
// All fields camelCase to match #[serde(rename_all = "camelCase")]

export type Engine = 'groq';
export type RecordingMode = 'pushToTalk' | 'toggle';
export type InjectMethod = 'paste' | 'type';

export interface AppConfig {
  engine: Engine;
  groqModel: string;
  language: string;
  hotkey: string;
  mode: RecordingMode;
  injectMethod: InjectMethod;
  enhanceEnabled: boolean;
  enhancePrompt: string;
  enhanceModel: string;
  launchAtStartup: boolean;
}

export type RecordingState = 'idle' | 'recording' | 'transcribing';

export interface Transcription {
  id: string;
  createdAt: string;
  text: string;
  durationMs: number;
  engine: string;
  language: string;
  targetApp: string | null;
  enhanced: number; // 0 | 1 (SQLite bool)
}

export interface EngineStatus {
  engine: string;
  ok: boolean;
  message: string;
}

// Event payloads — mirror of src-tauri/src/events.rs
export interface RecordingResultPayload {
  text: string;
}

export interface RecordingErrorPayload {
  message: string;
}

export interface EngineChangedPayload {
  engine: string;
}

// Partial config for update_settings command
export type PartialConfig = Partial<AppConfig>;
