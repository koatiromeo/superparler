#!/usr/bin/env bash
set -euo pipefail

MODELS_DIR="$(cd "$(dirname "$0")/.." && pwd)/models"
mkdir -p "$MODELS_DIR"

# ─── Whisper ggml-small.bin ─────────────────────────────────────────────────
WHISPER_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
WHISPER_FILE="$MODELS_DIR/ggml-small.bin"
WHISPER_SIZE_MB=466

if [ -f "$WHISPER_FILE" ]; then
  echo "✓ ggml-small.bin already present ($(du -sh "$WHISPER_FILE" | cut -f1))"
else
  echo "↓ Downloading ggml-small.bin (~${WHISPER_SIZE_MB} MB)..."
  curl -L --progress-bar -o "$WHISPER_FILE" "$WHISPER_URL"
  echo "✓ ggml-small.bin downloaded"
fi

# ─── Silero VAD onnx ────────────────────────────────────────────────────────
SILERO_URL="https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"
SILERO_FILE="$MODELS_DIR/silero_vad.onnx"

if [ -f "$SILERO_FILE" ]; then
  echo "✓ silero_vad.onnx already present"
else
  echo "↓ Downloading silero_vad.onnx..."
  curl -L --progress-bar -o "$SILERO_FILE" "$SILERO_URL"
  echo "✓ silero_vad.onnx downloaded"
fi

echo ""
echo "Models ready in: $MODELS_DIR"
ls -lh "$MODELS_DIR"
