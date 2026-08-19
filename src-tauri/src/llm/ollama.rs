//! Ollama backend – POST /api/chat se stream=true (NDJSON).

use super::{LlmProvider, TokenSink};
use crate::config::OllamaConfig;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;

pub struct Ollama {
    url: String,
    model: String,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct ChatChunk {
    message: Option<ChunkMsg>,
    #[serde(default)]
    done: bool,
    error: Option<String>,
}
#[derive(Deserialize)]
struct ChunkMsg {
    content: String,
}

impl Ollama {
    pub fn new(c: &OllamaConfig) -> Self {
        Self {
            url: c.url.trim_end_matches('/').to_string(),
            model: c.model.clone(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for Ollama {
    async fn complete(&self, system: &str, user: &str, sink: TokenSink<'_>) -> Result<String> {
        let body = json!({
            "model": self.model,
            "stream": true,
            // Gemma 4 / Qwen 3 mají "thinking" – pro hotkey akce ho vypínáme (jinak minuty úvah).
            "think": false,
            "options": { "temperature": 0.2 },
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ]
        });
        let resp = self
            .http
            .post(format!("{}/api/chat", self.url))
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("Ollama nedostupná ({}): {e}", self.url))?;
        if !resp.status().is_success() {
            let s = resp.status();
            let t = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama HTTP {s}: {t}"));
        }

        let mut out = String::new();
        let mut buf = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            // NDJSON – zpracovat celé řádky, zbytek nechat v bufferu
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf.drain(..=pos);
                if line.is_empty() {
                    continue;
                }
                let c: ChatChunk = serde_json::from_str(&line)?;
                if let Some(e) = c.error {
                    return Err(anyhow!("Ollama: {e}"));
                }
                if let Some(m) = c.message {
                    if !m.content.is_empty() {
                        sink(&m.content);
                        out.push_str(&m.content);
                    }
                }
                if c.done {
                    return Ok(out);
                }
            }
        }
        Ok(out)
    }

    async fn health(&self) -> Result<String> {
        let v: serde_json::Value = self
            .http
            .get(format!("{}/api/tags", self.url))
            .send()
            .await
            .map_err(|e| anyhow!("Ollama nedostupná: {e}"))?
            .json()
            .await?;
        let models: Vec<String> = v["models"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        if models.iter().any(|m| m == &self.model || m.trim_end_matches(":latest") == self.model) {
            Ok(format!("OK – model {} je stažený", self.model))
        } else {
            Ok(format!("Ollama běží, ale model {} chybí. Dostupné: {}", self.model, models.join(", ")))
        }
    }
}
