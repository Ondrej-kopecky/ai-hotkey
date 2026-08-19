//! Akce = pojmenovaný prompt, který se aplikuje na označený text.
//! Prompt může obsahovat placeholdery `{lang}` (cílový jazyk překladu) a `{out}` (jazyk výstupu).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub name: String,
    /// Ikona v kolečku: "translate" | "list" | "pen" | "check" | "bulb" | "spark" (jinak spark)
    #[serde(default)]
    pub icon: String,
    /// System prompt – co má model dělat. Bez `{text}`; text jde jako user message.
    pub prompt: String,
    /// "replace" = výsledek rovnou nahradí označený text; "show" = jen zobrazit v popupu
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Písmeno pro výběr v kolečku (jeden znak, např. "g"). Prázdné = jen číslo/šipky.
    #[serde(default)]
    pub key: String,
    /// Volitelná globální zkratka, která akci spustí rovnou bez menu (např. "Ctrl+Shift+G").
    #[serde(default)]
    pub hotkey: String,
    /// Vypnutá akce se v kolečku nezobrazuje (zůstává v configu).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Krátký popis pro kartu v Nastavení.
    #[serde(default)]
    pub description: String,
    /// Pevný model pro tuto akci: "" = výchozí, jinak "ollama:<název>" nebo "anthropic:<id>".
    #[serde(default)]
    pub model: String,
}

fn default_true() -> bool {
    true
}

fn default_mode() -> String {
    "show".into()
}

impl Action {
    pub fn builtin() -> Vec<Action> {
        vec![
            Action {
                id: "fix".into(),
                enabled: true,
                description: "Opraví pravopis, gramatiku a interpunkci, zachová význam i jazyk.".into(),
                model: String::new(),
                key: "g".into(),
                hotkey: String::new(),
                name: "Opravit gramatiku".into(),
                icon: "check".into(),
                prompt: "You are a proofreader. Fix spelling, grammar and punctuation in the user's text. \
                         Do NOT translate – keep exactly the same language as the input (usually {out}). \
                         Keep meaning, tone and formatting. \
                         Output ONLY the corrected text, no explanations, no quotes.".into(),
                mode: "replace".into(),
            },
            Action {
                id: "translate".into(),
                enabled: true,
                description: "Přeloží text do výchozího jazyka překladu (z něj zpět do češtiny).".into(),
                model: String::new(),
                key: "p".into(),
                hotkey: String::new(),
                name: "Přeložit".into(),
                icon: "translate".into(),
                prompt: "You are a translator. First silently detect the language of the user's text. \
                         RULE: if the text is written in {lang}, translate it into {out}. \
                         If the text is written in any other language (including {out}), translate it into {lang}. \
                         Never return the text in its original language. \
                         Preserve formatting, tone and idioms. Output ONLY the translation, no explanations, no quotes.".into(),
                mode: "show".into(),
            },
            Action {
                id: "summarize".into(),
                enabled: true,
                description: "Stručně shrne text, u delšího do odrážek.".into(),
                model: String::new(),
                key: "s".into(),
                hotkey: String::new(),
                name: "Shrnout".into(),
                icon: "list".into(),
                prompt: "Summarize the user's text concisely. Write the summary in {out}. \
                         Use short bullet points if the text is long. Output ONLY the summary.".into(),
                mode: "show".into(),
            },
            Action {
                id: "tone".into(),
                enabled: true,
                description: "Profesionálně a zdvořile přeformuluje vybraný text.".into(),
                model: String::new(),
                key: "t".into(),
                hotkey: String::new(),
                name: "Upravit styl".into(),
                icon: "pen".into(),
                prompt: "Rewrite the user's text in a polite, professional tone. Write the result in {out} \
                         (do NOT translate into any other language). Keep the meaning. \
                         Output ONLY the rewritten text, no explanations, no quotes.".into(),
                mode: "replace".into(),
            },
            Action {
                id: "explain".into(),
                enabled: true,
                description: "Jednoduše vysvětlí, co označený text znamená (pojem, věta, kód, chybová hláška).".into(),
                model: String::new(),
                key: "v".into(),
                hotkey: String::new(),
                name: "Vysvětlit".into(),
                icon: "bulb".into(),
                prompt: "Explain the user's text clearly and concisely in {out}. It may be a term, a sentence, \
                         a piece of code, or an error message. Start with a one-sentence plain-language explanation, \
                         then add at most 3 short bullet points with useful context or an example. \
                         Output ONLY the explanation, no preamble.".into(),
                mode: "show".into(),
            },
        ]
    }

    /// Doplní placeholdery do system promptu.
    pub fn render_prompt(&self, lang: &str, out: &str) -> String {
        self.prompt.replace("{lang}", lang).replace("{out}", out)
    }
}
