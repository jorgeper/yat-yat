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
///
/// It never follows redirects: the endpoint check covers the URL we were
/// given, and a redirect would let whatever answers on localhost forward the
/// request — transcript included — to any host. A redirect response is just
/// a non-success status here, i.e. "enhancement skipped".
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("building the enhancement HTTP client")
    })
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

    /// Answer every request on a throwaway localhost port with `response`;
    /// returns the port and a counter of requests received.
    fn serve(response: String) -> (u16, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hits2 = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                hits2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (port, hits)
    }

    // R38: a redirect from the local endpoint is never followed — the
    // transcript goes to the URL the guard approved and nowhere else.
    #[tokio::test]
    async fn r38_enhance_never_follows_a_redirect() {
        let body = r#"{"message":{"content":"leaked"}}"#;
        let (elsewhere, elsewhere_hits) = serve(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        // 307 keeps the method and body: followed, it would re-POST the text.
        let (local, local_hits) = serve(format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{elsewhere}/api/chat\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ));
        let cfg = EnhancementSettings {
            enabled: true,
            endpoint: format!("http://127.0.0.1:{local}"),
            ..Default::default()
        };
        let err = enhance("private dictation", &cfg).await.unwrap_err();
        assert!(err.to_string().contains("307"), "{err}");
        assert_eq!(local_hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            elsewhere_hits.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the redirect target must never be contacted"
        );
        // Same for the warmup request.
        warmup(&cfg).await;
        assert_eq!(elsewhere_hits.load(std::sync::atomic::Ordering::SeqCst), 0);
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
