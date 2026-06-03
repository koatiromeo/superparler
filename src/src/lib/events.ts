// Typed listen() wrappers for all events emitted from Rust.
// Event names must match constants in src-tauri/src/events.rs
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { EngineChangedPayload, RecordingErrorPayload, RecordingResultPayload } from '../types';

export const onRecordingStarted = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('recording:started', () => cb());

export const onRecordingStopped = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('recording:stopped', () => cb());

export const onRecordingTranscribing = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('recording:transcribing', () => cb());

export const onRecordingResult = (cb: (text: string) => void): Promise<UnlistenFn> =>
  listen<RecordingResultPayload>('recording:result', e => cb(e.payload.text));

export const onRecordingError = (cb: (message: string) => void): Promise<UnlistenFn> =>
  listen<RecordingErrorPayload>('recording:error', e => cb(e.payload.message));

export const onEngineChanged = (cb: (engine: string) => void): Promise<UnlistenFn> =>
  listen<EngineChangedPayload>('engine:changed', e => cb(e.payload.engine));
