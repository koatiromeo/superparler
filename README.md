# SuperParler

Desktop dictation app — global hotkey → voice capture → transcription → text injected at cursor.

Alternative to SuperWhisper, built entirely in Rust with Tauri 2. Offline-first, cross-platform.

## Pipeline

```
hotkey ──► cpal capture ──► rubato 16kHz mono ──► Silero VAD trim
                                                        │
                                              ┌─────────▼──────────┐
                                              │  Transcriber trait  │
                                              │  ├─ LocalWhisper    │ (offline, default)
                                              │  └─ GroqWhisper     │ (cloud boost)
                                              └─────────┬──────────┘
                                                        │
                                              enhance? (Groq LLM, v2)
                                                        │
                                              inject (clipboard + paste)
                                                        │
                                              SQLite persist + emit event
```

## Quick Start

```bash
# 1. Prerequisites
make setup          # rustup + system deps + npm install

# 2. Models (one-time, ~470 MB)
make models         # ggml-small.bin + silero_vad.onnx → models/

# 3. Development
make dev            # cargo tauri dev + Vite hot-reload

# 4. Production bundle
make build          # → src-tauri/target/release/bundle/
```

## Tech Stack

| Layer     | Tech                                        |
|-----------|---------------------------------------------|
| Shell     | Tauri 2 (WebView + Rust backend)            |
| Audio     | cpal (capture) · rubato (resample)          |
| VAD       | Silero via voice_activity_detector/ort      |
| STT Local | whisper-rs (GGUF bindings)                  |
| STT Cloud | Groq API — whisper-large-v3-turbo           |
| Injection | enigo (keyboard simulation) + clipboard     |
| Storage   | sqlx + SQLite                               |
| Frontend  | Vite + React 19 + TypeScript + Tailwind     |

## Platform Permissions

| OS      | Permission required            | Where to grant                                              |
|---------|--------------------------------|-------------------------------------------------------------|
| macOS   | Accessibility + Microphone     | System Settings → Privacy & Security                        |
| Windows | None                           | —                                                           |
| Linux   | Microphone (PulseAudio/ALSA)   | System settings; Wayland: install ydotool + run ydotoold   |

## Configuration

Settings are stored in:
- **macOS**: `~/Library/Application Support/com.nemaleu.superparler/config.toml`
- **Windows**: `%APPDATA%\nemaleu\superparler\config\config.toml`
- **Linux**: `~/.config/superparler/config.toml`

Groq API key: stored in OS keyring (never in config file).

## Development Commands

```bash
make dev          # Start dev server (Rust + Vite)
make test-rust    # cargo test --lib
make lint         # cargo clippy -D warnings
make fmt          # cargo fmt
make ci           # Full local CI (fmt-check + lint + test)
make doc          # cargo doc --open
```

## Project Structure

```
superparler/
├── src-tauri/      Rust backend (Tauri 2)
│   ├── src/
│   │   ├── pipeline.rs     Orchestration (capture→vad→stt→inject→persist)
│   │   ├── stt/            Transcriber trait + LocalWhisper + GroqWhisper
│   │   ├── audio/          cpal capture + rubato resample + Silero VAD
│   │   ├── inject/         Platform-specific text injection
│   │   ├── commands/       Tauri IPC handlers (thin, no business logic)
│   │   └── storage/        SQLite via sqlx
│   └── migrations/         SQL migrations
├── src/            Frontend (Vite + React + TypeScript + Tailwind)
│   └── src/
│       ├── lib/tauri.ts    Typed invoke() wrappers
│       ├── lib/events.ts   Typed event listeners
│       ├── types/index.ts  TypeScript mirror of all Rust IPC structs
│       ├── stores/         Zustand (settings, recording state)
│       └── components/     Settings page + History page
├── models/         GGUF + ONNX models (git-ignored, downloaded via make models)
└── scripts/        setup.sh + download-models.sh
```

## License

MIT — © Nemaleu Engine Core
