# ai-hotkey – kontext pro AI asistenty

Desktopová appka: označ text kdekoli → globální zkratka → radiální menu akcí → LLM
(Ollama lokálně / Anthropic API) → výsledek nahradí výběr nebo se zkopíruje.
Tauri v2 (Rust) + vanilla TypeScript.

- Architektura, tok dat, commandy, eventy: `docs/ARCHITECTURE.md`
- Zadání pro Linux port: `docs/LINUX-PORT.md`
- OS-specifický kód **jen** v `src-tauri/src/platform/` (trait `Platform`); ostatní vrstvy držet
  platformně neutrální.
- Uživatelský config: `%APPDATA%\ai-hotkey\config.json` (Linux `~/.config/ai-hotkey/config.json`).
- Dev: `npm run tauri dev`; release: `npm run tauri build` (NSIS). Před `cargo build` ukončit
  běžící `ai-hotkey.exe` (drží binárku).
- Thinking modely (Gemma 4, Qwen 3): Ollama `/api/chat` dostává `think:false`; pro OpenAI klienty
  (Brave Leo) slouží vestavěný bridge na :11435 (`src-tauri/src/bridge.rs`).
