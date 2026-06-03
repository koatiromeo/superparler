// TODO(v2): GroqLlm enhancer — reformats transcription via Groq chat completions.
//
// impl Enhancer for GroqLlm:
//   - POST to https://api.groq.com/openai/v1/chat/completions
//   - model: e.g. "llama-3.1-8b-instant"
//   - system: "Tu es un assistant qui reformate du texte transcrit."
//   - user: "{prompt}\n\nTexte: {text}"
//   - Extract choices[0].message.content
//   - API key via keyring (same entry as GroqWhisper)
//
// Blocked on: user-facing prompt configuration in Settings UI
