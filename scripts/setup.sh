#!/usr/bin/env bash
set -euo pipefail

echo "=== SuperParler Setup ==="

# ─── Rust ───────────────────────────────────────────────────────────────────
if ! command -v rustup &>/dev/null; then
  echo "→ Installing rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
else
  echo "✓ rustup $(rustup --version 2>&1 | head -1)"
fi

rustup update stable
rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
rustup target add aarch64-apple-darwin 2>/dev/null || true
rustup target add x86_64-apple-darwin 2>/dev/null || true

# ─── Tauri CLI ──────────────────────────────────────────────────────────────
if ! cargo tauri --version &>/dev/null 2>&1; then
  echo "→ Installing @tauri-apps/cli..."
  cargo install tauri-cli --version "^2"
fi

# ─── System deps by OS ──────────────────────────────────────────────────────
OS="$(uname -s)"

if [ "$OS" = "Linux" ]; then
  echo "→ Installing Linux system dependencies..."
  if command -v apt-get &>/dev/null; then
    sudo apt-get update
    sudo apt-get install -y \
      libasound2-dev \
      libwebkit2gtk-4.1-dev \
      libssl-dev \
      libgtk-3-dev \
      libayatana-appindicator3-dev \
      librsvg2-dev \
      cmake \
      clang \
      libclang-dev
  elif command -v dnf &>/dev/null; then
    sudo dnf install -y \
      alsa-lib-devel \
      webkit2gtk4.1-devel \
      openssl-devel \
      gtk3-devel \
      cmake \
      clang
  else
    echo "! Unsupported Linux distro — install dependencies manually (see README)"
  fi

elif [ "$OS" = "Darwin" ]; then
  echo "→ macOS: ensuring Xcode Command Line Tools (cmake, clang included)..."
  xcode-select --install 2>/dev/null || true
  echo "  Note: allow SuperParler in System Settings → Privacy → Accessibility & Microphone"

elif [[ "$OS" == MINGW* ]] || [[ "$OS" == CYGWIN* ]]; then
  echo "→ Windows: install Build Tools for Visual Studio + cmake manually if not present"
  echo "  See: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
fi

# ─── Frontend deps ──────────────────────────────────────────────────────────
echo "→ Installing frontend npm dependencies..."
(cd "$(dirname "$0")/../src" && npm install)

echo ""
echo "=== Setup complete ==="
echo "Next steps:"
echo "  make models   # download ggml-small.bin + silero_vad.onnx"
echo "  make dev      # start development server"
