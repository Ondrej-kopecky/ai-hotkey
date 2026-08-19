//! Anthropic Messages API backend (volitelný cloud). Streaming přes SSE.

use super::{LlmProvider, TokenSink};
use crate::config::AnthropicConfig;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};

pub struct Anthropic {
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl Anthropic {
    pub fn new(c: &AnthropicConfig) -> Self {
        Self { api_key: c.api_key.clone(), model: c.model.clone(), http: reqwest::Client::new() }
    }
}

#[async_trait]
impl LlmProvider for Anthropic {
    async fn complete(&self, system: &str, user: &str, sink: TokenSink<'_>) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow!("Anthropic API klíč není nastavený"));
        }
        let mut body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "stream": true,
            "system": system,
            "messages": [{ "role": "user", "content": user }]
        });
        // Hotkey akce mají být rychlé: Sonnet 5 bez thinkingu, Opus 5 s nízkým effortem
        // (thinking na Opus 5 nevypínáme – doporučení API), Haiku effort nepodporuje.
        if self.model.starts_with("claude-sonnet-5") {
            body["thinking"] = json!({ "type": "disabled" });
        } else if self.model.starts_with("claude-opus") {
            body["output_config"] = json!({ "effort": "low" });
        }
        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let t = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic HTTP {s}: {t}"));
        }
        let mut out = String::new();
        let mut buf = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            buf.push_str(&String::from_utf8_lossy(&chunk?));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf.drain(..=pos);
                if let Some(data) = line.strip_prefix("data: ") {
                    let v: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if v["type"] == "content_block_delta" {
                        if let Some(t) = v["delta"]["text"].as_str() {
                            sink(t);
                            out.push_str(t);
                        }
                    } else if v["type"] == "error" {
                        return Err(anyhow!("Anthropic: {}", v["error"]["message"]));
                    }
                }
            }
        }
        Ok(out)
    }

    async fn health(&self) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow!("API klíč není nastavený"));
        }
        Ok(format!("Klíč nastaven, model {}", self.model))
    }
}
