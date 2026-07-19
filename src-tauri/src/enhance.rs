//! Optional LLM enhancement (SPEC §4.2): localhost-only Ollama, OFF by default,
//! 5 s timeout falling back to the deterministically-filtered text.

use crate::settings::{validate_enhancement_endpoint, EnhancementSettings};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::sync::OnceLock;
use std::time::Duration;

pub const ENHANCE_TIMEOUT: Duration = Duration::from_secs(5);
const WARMUP_TIMEOUT: Duration = Duration::from_millis(1500);

/// One shared client for warmup + enhance (SPEC14 FR-D6): the warmup's
/// kept-alive localhost connection then serves the enhance call on the stop
/// path. Timeouts are per-request, not per-client.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

fn chat_url(endpoint: &str) -> String {
    format!("{}/api/chat", endpoint.trim_end_matches('/'))
}

/// Rewrite `text` through the configured local model. Errors are the caller's
/// signal to fall back to the filtered text (SPEC: "enhancement skipped").
pub async fn enhance(text: &str, cfg: &EnhancementSettings) -> Result<String> {
    validate_enhancement_endpoint(&cfg.endpoint)?;
    let body = json!({
        "model": cfg.model,
        "stream": false,
        "options": { "temperature": 0.2 },
        "messages": [
            { "role": "system", "content": cfg.prompt },
            { "role": "user", "content": text },
        ],
    });
    let response = client()
        .post(chat_url(&cfg.endpoint))
        .timeout(ENHANCE_TIMEOUT)
        .json(&body)
        .send()
        .await
        .context("reaching enhancement endpoint")?;
    if !response.status().is_success() {
        bail!("enhancement endpoint returned HTTP {}", response.status());
    }
    let value: serde_json::Value = response.json().await.context("parsing response")?;
    let content = value["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        bail!("enhancement returned empty text");
    }
    Ok(content)
}

/// Ask Ollama to load the model into memory (fired when recording starts so a
/// cold model never queues the paste — SPEC §4.2).
pub async fn warmup(cfg: &EnhancementSettings) {
    if validate_enhancement_endpoint(&cfg.endpoint).is_err() {
        return;
    }
    // An empty-message chat request loads the model and returns immediately.
    let _ = client()
        .post(chat_url(&cfg.endpoint))
        .timeout(WARMUP_TIMEOUT)
        .json(&json!({ "model": cfg.model, "messages": [] }))
        .send()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enhance_rejects_remote_endpoint_before_any_io() {
        let cfg = EnhancementSettings {
            enabled: true,
            endpoint: "https://api.example.com".into(),
            ..Default::default()
        };
        let err = enhance("hi", &cfg).await.unwrap_err();
        assert!(err.to_string().contains("localhost"), "{err}");
    }

    #[tokio::test]
    async fn enhance_fails_cleanly_when_no_server() {
        // Port 1 is never an Ollama server; the error is the fallback signal.
        let cfg = EnhancementSettings {
            enabled: true,
            endpoint: "http://127.0.0.1:1".into(),
            ..Default::default()
        };
        assert!(enhance("hi", &cfg).await.is_err());
    }
}
