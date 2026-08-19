//! Nastavení aplikace – ukládá se jako JSON v `dirs::config_dir()/ai-hotkey/config.json`.
//! (Windows: %APPDATA%\ai-hotkey\config.json, Linux: ~/.config/ai-hotkey/config.json)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::actions::Action;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Globální zkratka, která otevře popup (formát tauri-plugin-global-shortcut, např. "Ctrl+Shift+Space").
    pub hotkey: String,
    /// Který LLM backend používat: "ollama" | "anthropic"
    pub provider: String,
    pub ollama: OllamaConfig,
    pub anthropic: AnthropicConfig,
    /// Seznam akcí (vestavěné + vlastní). Uživatel může editovat.
    pub actions: Vec<Action>,
    /// Do jakého jazyka překládat (akce "translate") – placeholder `{lang}`.
    pub target_language: String,
    /// V jakém jazyce mají být výstupy ostatních akcí (shrnutí, tón…) – placeholder `{out}`.
    pub output_language: String,
    /// true = akce v režimu "replace" vloží výsledek hned bez potvrzení.
    pub auto_replace: bool,
    /// Lokální proxy pro Brave Leo (a jiné OpenAI-kompatibilní klienty): přeposílá na Ollamu
    /// a vypíná thinking (`reasoning_effort: none`).
    pub leo_bridge: LeoBridgeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LeoBridgeConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for LeoBridgeConfig {
    fn default() -> Self {
        Self { enabled: true, port: 11435 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnthropicConfig {
    /// Klíč žije jen v paměti (načtený z trezoru OS) – NIKDY se neserializuje do config.json
    /// ani neposílá do frontendu. Frontend ho naopak může poslat (nový klíč k uložení).
    #[serde(skip_serializing)]
    pub api_key: String,
    /// Informativní příznak pro UI: klíč je uložený v trezoru.
    pub api_key_stored: bool,
    pub model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self { url: "http://localhost:11434".into(), model: "gemma4:12b".into() }
    }
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self { api_key: String::new(), api_key_stored: false, model: "claude-sonnet-5".into() }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+Space".into(),
            provider: "ollama".into(),
            ollama: OllamaConfig::default(),
            anthropic: AnthropicConfig::default(),
            actions: Action::builtin(),
            target_language: "English".into(),
            output_language: "Czech".into(),
            auto_replace: false,
            leo_bridge: LeoBridgeConfig::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("ai-hotkey");
        p.push("config.json");
        p
    }

    pub fn load() -> Self {
        let path = Self::path();
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(s) => match serde_json::from_str::<Config>(&s) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("config.json je poškozený ({e}), používám default");
                    Config::default()
                }
            },
            Err(_) => {
                let c = Config::default();
                let _ = c.save();
                c
            }
        };
        // Migrace: klíč v plain textu ve starém config.json → přesunout do trezoru a z JSONu smazat.
        if !cfg.anthropic.api_key.is_empty() {
            match crate::secrets::set(crate::secrets::ANTHROPIC_KEY, &cfg.anthropic.api_key) {
                Ok(()) => {
                    log::info!("Anthropic API klíč přesunut z config.json do trezoru OS");
                    cfg.anthropic.api_key_stored = true;
                    let _ = cfg.save(); // api_key se neserializuje → z JSONu zmizí
                }
                Err(e) => log::warn!("migrace klíče do trezoru selhala: {e}"),
            }
        } else {
            cfg.anthropic.api_key = crate::secrets::get(crate::secrets::ANTHROPIC_KEY).unwrap_or_default();
            cfg.anthropic.api_key_stored = !cfg.anthropic.api_key.is_empty();
        }
        cfg
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
