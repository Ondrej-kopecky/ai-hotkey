//! LLM backendy. Každý implementuje [`LlmProvider`]; streamované tokeny se posílají
//! přes callback, aby UI mohlo výsledek vykreslovat průběžně.

use anyhow::Result;
use async_trait::async_trait;

pub mod anthropic;
pub mod ollama;

/// Callback pro streamované kousky textu.
pub type TokenSink<'a> = &'a (dyn Fn(&str) + Send + Sync);

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Pošle system + user prompt, streamuje odpověď do `sink`, vrátí celý text.
    async fn complete(&self, system: &str, user: &str, sink: TokenSink<'_>) -> Result<String>;

    /// Rychlá kontrola dostupnosti (pro Nastavení / diagnostiku).
    async fn health(&self) -> Result<String>;
}

pub fn from_config(cfg: &crate::config::Config) -> Box<dyn LlmProvider> {
    from_spec(cfg, &default_spec(cfg))
}

/// Identifikátor modelu ve tvaru "ollama:<název>" | "anthropic:<id>".
pub fn default_spec(cfg: &crate::config::Config) -> String {
    match cfg.provider.as_str() {
        "anthropic" => format!("anthropic:{}", cfg.anthropic.model),
        _ => format!("ollama:{}", cfg.ollama.model),
    }
}

/// Provider podle specu; neplatný/prázdný spec = výchozí z configu.
pub fn from_spec(cfg: &crate::config::Config, spec: &str) -> Box<dyn LlmProvider> {
    if let Some(m) = spec.strip_prefix("anthropic:") {
        let mut c = cfg.anthropic.clone();
        c.model = m.to_string();
        return Box::new(anthropic::Anthropic::new(&c));
    }
    if let Some(m) = spec.strip_prefix("ollama:") {
        let mut c = cfg.ollama.clone();
        c.model = m.to_string();
        return Box::new(ollama::Ollama::new(&c));
    }
    match cfg.provider.as_str() {
        "anthropic" => Box::new(anthropic::Anthropic::new(&cfg.anthropic)),
        _ => Box::new(ollama::Ollama::new(&cfg.ollama)),
    }
}

/// Modely dostupné pro rychlé přepínání (Ollama tags + Claude, pokud je klíč).
pub const ANTHROPIC_MODELS: &[(&str, &str)] = &[
    ("claude-sonnet-5", "Claude Sonnet 5"),
    ("claude-opus-5", "Claude Opus 5"),
    ("claude-haiku-4-5", "Claude Haiku 4.5"),
];
