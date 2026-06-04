use async_trait::async_trait;
use serde::Deserialize;

use super::Enhancer;
use crate::error::{AppError, Result};

const GROQ_CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const KEYRING_SERVICE: &str = "superparler";
const KEYRING_ACCOUNT: &str = "groq";

pub struct GroqLlm {
    model: String,
    client: reqwest::Client,
}

impl GroqLlm {
    pub fn new(model: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;
        Ok(Self { model, client })
    }

    /// Priority: OS keyring ("superparler"/"groq") → env var GROQ_API_KEY.
    fn get_api_key() -> Result<String> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
            if let Ok(key) = entry.get_password() {
                if !key.trim().is_empty() {
                    return Ok(key);
                }
            }
        }
        std::env::var("GROQ_API_KEY").map_err(|_| {
            AppError::Keyring(
                "Groq API key not found. Set it via:\n\
                 • app Settings → Moteur → Clé API (stored in OS keyring), or\n\
                 • environment variable GROQ_API_KEY"
                    .to_string(),
            )
        })
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

#[async_trait]
impl Enhancer for GroqLlm {
    async fn enhance(&self, text: &str, prompt: &str) -> Result<String> {
        let api_key = Self::get_api_key()?;

        let system_msg = "Tu es un assistant qui reformate du texte transcrit. \
                          Retourne uniquement le texte reformaté, sans explication ni commentaire.";

        let user_msg = if prompt.trim().is_empty() {
            format!("Corrige et formate ce texte transcrit :\n\n{text}")
        } else {
            format!("{prompt}\n\nTexte : {text}")
        };

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_msg},
                {"role": "user",   "content": user_msg}
            ],
            "temperature": 0.3,
            "max_tokens": 2048
        });

        tracing::info!(model = %self.model, text_len = text.len(), "sending text to Groq LLM for enhancement");

        let response = self
            .client
            .post(GROQ_CHAT_URL)
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Groq enhance request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(AppError::Enhance(format!("Groq LLM API {status}: {body_text}")));
        }

        let chat: ChatResponse = response
            .json()
            .await
            .map_err(|e| AppError::Enhance(format!("Groq LLM JSON parse: {e}")))?;

        let enhanced = chat
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Enhance("Groq LLM: no choices in response".to_string()))?
            .message
            .content
            .trim()
            .to_string();

        tracing::info!(enhanced_len = enhanced.len(), "Groq LLM enhancement complete");
        Ok(enhanced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_llm_new() {
        let llm = GroqLlm::new("llama-3.3-70b-versatile".to_string());
        assert!(llm.is_ok());
    }

    #[test]
    fn test_get_api_key_falls_back_to_env() {
        // SAFETY: single-threaded test; no other threads read GROQ_API_KEY concurrently.
        unsafe { std::env::set_var("GROQ_API_KEY", "test_key_enhance") };
        let key = GroqLlm::get_api_key();
        unsafe { std::env::remove_var("GROQ_API_KEY") };
        assert!(key.is_ok(), "get_api_key must succeed when GROQ_API_KEY is set");
    }

    #[tokio::test]
    #[ignore = "network — requires GROQ_API_KEY env var or keyring"]
    async fn test_groq_llm_enhance() {
        let llm = GroqLlm::new("llama-3.3-70b-versatile".to_string()).unwrap();
        let result = llm
            .enhance(
                "bonjour je voudrais savoir si vous pouvez m aider avec mon probleme",
                "corrige et formate en email professionnel",
            )
            .await;
        match result {
            Ok(text) => println!("Enhanced: «{text}»"),
            Err(e) => panic!("Groq LLM enhance failed: {e}"),
        }
    }
}
