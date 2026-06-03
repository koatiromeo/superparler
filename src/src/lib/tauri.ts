// Typed wrappers for ALL Tauri invoke() calls.
// Components MUST use these — never call invoke() directly.
import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  EngineStatus,
  PartialConfig,
  RecordingState,
  Transcription,
} from '../types';

// recording commands
export const startRecording = (): Promise<void> =>
  invoke('start_recording');

export const stopRecording = (): Promise<void> =>
  invoke('stop_recording');

export const getRecordingState = (): Promise<RecordingState> =>
  invoke('get_recording_state');

// settings commands
export const getSettings = (): Promise<AppConfig> =>
  invoke('get_settings');

export const updateSettings = (patch: PartialConfig): Promise<AppConfig> =>
  invoke('update_settings', { patch });

// history commands
export const listTranscriptions = (limit = 50, offset = 0): Promise<Transcription[]> =>
  invoke('list_transcriptions', { limit, offset });

export const deleteTranscription = (id: string): Promise<void> =>
  invoke('delete_transcription', { id });

export const clearHistory = (): Promise<void> =>
  invoke('clear_history');

// engine commands
export const testEngine = (): Promise<EngineStatus> =>
  invoke('test_engine');

export const listLocalModels = (): Promise<string[]> =>
  invoke('list_local_models');

export const setGroqKey = (key: string): Promise<void> =>
  invoke('set_groq_key', { key });

// system commands
export const setLaunchAtStartup = (enabled: boolean): Promise<void> =>
  invoke('set_launch_at_startup', { enabled });

export const openSettingsWindow = (): Promise<void> =>
  invoke('open_settings_window');
