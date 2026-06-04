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
                                              enhance? (Groq LLM)
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

# 4. Production bundle (native installer for your OS)
make release        # → src-tauri/target/release/bundle/
make run-release    # build + launch the packaged binary
```

## Tech Stack

| Layer     | Tech                                        |
|-----------|---------------------------------------------|
| Shell     | Tauri 2 (WebView + Rust backend)            |
| Audio     | cpal (capture) · rubato (resample)          |
| VAD       | Silero via voice_activity_detector/ort      |
| STT Local | whisper-rs (GGUF bindings)                  |
| STT Cloud | Groq API — whisper-large-v3-turbo           |
| LLM       | Groq API — llama-3.3-70b-versatile (enhance)|
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

## Tray Icon States

The system tray icon reflects the recording state:

| State | Icon | Tooltip |
|---|---|---|
| Idle | Grey circle | SuperParler — Prêt |
| Recording | Red circle | SuperParler — Enregistrement… |
| Transcribing | Orange circle | SuperParler — Transcription… |

## Development Commands

```bash
make dev          # Start dev server (Rust + Vite)
make test         # cargo test (lib + integration) + frontend typecheck
make test-rust    # cargo test --lib + --test pipeline_test
make lint         # cargo clippy -D warnings
make fmt          # cargo fmt
make ci           # Full local CI (fmt-check + lint + test)
make doc          # cargo doc --open
```

## Release Build

```bash
make release      # cargo tauri build → native bundle in src-tauri/target/release/bundle/
make run-release  # build debug bundle + launch
```

Output artifacts per platform:

| OS | Bundle |
|---|---|
| macOS | `target/release/bundle/dmg/SuperParler_*.dmg` |
| Windows | `target\release\bundle\msi\SuperParler_*.msi` |
| Linux | `target/release/bundle/appimage/super-parler_*.AppImage` |

### macOS Signing & Notarization

For distribution outside the App Store, you need a **Developer ID Application** certificate.

```bash
# 1. Export your Developer ID cert as .p12 from Keychain Access
# 2. Encode it for GitHub Secrets:
base64 -i DeveloperID.p12 | pbcopy

# 3. Set in repo Settings → Secrets and variables → Actions:
#    APPLE_CERTIFICATE            (base64 .p12)
#    APPLE_CERTIFICATE_PASSWORD   (p12 export password)
#    APPLE_SIGNING_IDENTITY       ("Developer ID Application: Name (TEAMID)")
#    APPLE_ID                     (your Apple ID email)
#    APPLE_PASSWORD               (app-specific password from appleid.apple.com)
#    APPLE_TEAM_ID                (10-char team ID from developer.apple.com)

# 4. Uncomment the APPLE_* env vars in .github/workflows/release.yml
```

For a **local unsigned build** (macOS only, your own machine):
```bash
# Gatekeeper will block distribution — for personal use only
codesign --force --deep --sign - target/release/bundle/macos/SuperParler.app
```

**entitlements.plist** is included at `src-tauri/entitlements.plist` and declares:
- `com.apple.security.device.audio-input` — microphone
- `com.apple.security.network.client` — Groq API outbound
- `com.apple.security.automation.apple-events` — text injection
- Hardened Runtime (`cs.allow-jit`, `cs.allow-unsigned-executable-memory`) — required for notarization

### Windows Signing

Authenticode signing suppresses the SmartScreen "Unknown publisher" warning.

```powershell
# 1. Encode your .pfx certificate:
[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx")) | Set-Clipboard

# 2. Set in repo Secrets:
#    WINDOWS_CERTIFICATE           (base64 .pfx)
#    WINDOWS_CERTIFICATE_PASSWORD  (pfx password)

# 3. Set bundle.windows.certificateThumbprint in tauri.conf.json:
Get-ChildItem Cert:\CurrentUser\My  # find your thumbprint

# 4. Uncomment the WINDOWS_* env vars in .github/workflows/release.yml
```

Without signing, the app still works — Windows Defender SmartScreen warns on first run.

## CI / Release workflow

```
git push                 # triggers CI (fmt + clippy + tests)
git tag v0.1.0
git push --tags          # triggers release.yml → builds for all platforms
                         # → creates a draft GitHub Release with all installers
```

The release workflow builds:
- macOS arm64 (Apple Silicon)
- macOS x86_64 (Intel)
- Linux x86_64 (AppImage + .deb)
- Windows x86_64 (.msi + NSIS .exe)

## Project Structure

```
superparler/
├── src-tauri/      Rust backend (Tauri 2)
│   ├── src/
│   │   ├── pipeline.rs     Orchestration (capture→vad→stt→enhance→inject→persist)
│   │   ├── stt/            Transcriber trait + LocalWhisper + GroqWhisper
│   │   ├── enhance/        Enhancer trait + GroqLlm
│   │   ├── audio/          cpal capture + rubato resample + Silero VAD
│   │   ├── inject/         Platform-specific text injection
│   │   ├── commands/       Tauri IPC handlers (thin, no business logic)
│   │   └── storage/        SQLite via sqlx
│   ├── entitlements.plist  macOS microphone + accessibility entitlements
│   ├── icons/              App icons (png, icns, ico)
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
