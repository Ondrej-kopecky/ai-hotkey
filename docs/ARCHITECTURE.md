# Architektura

Tauri v2 (Rust backend + WebView frontend ve vanilla TS). Jedna codebase, OS-specifika
izolovaná do `src-tauri/src/platform/`.

## Tok jedné akce

```
uživatel označí text v appce X, zmáčkne hotkey
  │
  ▼  tauri-plugin-global-shortcut → lib.rs::on_hotkey()
  ├─ platform.foreground_window()      → uložit HWND appky X (AppState.target)
  ├─ platform::grab_selection()        → záloha schránky, Ctrl+C, přečíst, obnovit
  └─ show_popup()                      → okno "popup" (440×440) vycentrované na kurzor, event `popup-open` {text, actions, auto_action}
  │
  ▼  frontend (main.ts / popup): radiální kolečko (SVG výseče), výběr myší / písmenem (Action.key) / 1-9 / šipky+Enter
     (auto_action = akce spuštěná přímou globální zkratkou Action.hotkey → kolečko se přeskočí)
  invoke("run_action", {actionId, text})
  ├─ llm::from_config() → LlmProvider (Ollama / Anthropic)
  └─ provider.complete(system, text, sink) → sink emituje event `llm-token` {seq, token, done, error}
  │
  ▼  frontend streamuje výsledek do <div class=result>
  ├─ mode = "replace" → po `done` automaticky invoke("paste_result")
  └─ mode = "show"    → Enter/„Nahradit" = paste_result, Ctrl+C = copy_result
  │
  ▼  lib.rs::paste_result → platform::paste_text()
      schránka ← výsledek, platform.focus_window(target), Ctrl+V, po 300 ms obnovit schránku
```

`seq` chrání před smícháním streamů, když uživatel odstartuje další akci dřív, než doběhne
předchozí.

## Soubory

| Soubor | Co dělá |
|---|---|
| `src-tauri/src/lib.rs` | Bootstrap Tauri, tray, hotkey, okna, Tauri commandy, `AppState` |
| `src-tauri/src/config.rs` | `Config` (JSON v config dir), load/save, defaulty |
| `src-tauri/src/bridge.rs` | **Leo bridge** – axum proxy 127.0.0.1:11435 → Ollama, do `/v1/chat/completions` doplní `reasoning_effort: "none"` (vypne thinking Gemma 4 pro Brave Leo a jiné OpenAI klienty) |
| `src-tauri/src/secrets.rs` | API klíče v trezoru OS (`keyring`: Credential Manager / Keychain / Secret Service); config.json klíč neobsahuje, `api_key` se neserializuje, migrace ze starého configu při startu |
| `src-tauri/src/actions.rs` | `Action` + 5 vestavěných akcí, render `{lang}`/`{out}` |
| `src-tauri/src/llm/mod.rs` | trait `LlmProvider { complete(system, user, sink), health() }` |
| `src-tauri/src/llm/ollama.rs` | `/api/chat` stream=true (NDJSON) |
| `src-tauri/src/llm/anthropic.rs` | Messages API, SSE stream |
| `src-tauri/src/platform/mod.rs` | trait `Platform` + OS-nezávislé `grab_selection` / `paste_text` |
| `src-tauri/src/platform/windows.rs` | Win32 `GetForegroundWindow`/`SetForegroundWindow`, enigo klávesy |
| `src-tauri/src/platform/linux.rs` | **stub** – viz LINUX-PORT.md |
| `src-tauri/tauri.conf.json` | okno `popup` (frameless, transparent, alwaysOnTop, skipTaskbar, hidden) |
| `src-tauri/capabilities/default.json` | oprávnění pro okna `popup` a `settings` |
| `src/main.ts` | Frontend – `#popup` (kolečko + výsledkový panel s diffem) |
| `src/settings.ts` | Nastavení – levá navigace, karty akcí, editor akce (dialog) |
| `src/shared.ts` | Sdílené typy, ikony (`ICONS`), `esc` |
| `src/markdown.ts` | Mini Markdown → HTML (panel) / plain (schránka) |
| `src/styles.css` | Styly |
| `.github/workflows/release.yml` | CI: tag `v*` → NSIS instalátor do GitHub Release |

## Tauri commandy (Rust → volané z TS přes `invoke`)

| Command | Args | Vrací |
|---|---|---|
| `get_config` | – | `Config` |
| `save_config` | `{config}` | – (přeregistruje hotkey, když se změnila) |
| `check_provider` | – | string s výsledkem health checku |
| `run_action` | `{actionId, text?, model?}` | `seq` (u64); výsledek přes event `llm-token`; `model` = spec override (jinak `Action.model`, jinak výchozí) |
| `paste_result` | `{text}` | – (schová popup, vloží do původního okna) |
| `copy_result` | `{text}` | – |
| `close_popup` | – | – |
| `open_settings` | – | – (otevře/ukáže okno `settings`) |
| `resize_popup` | `{width, height}` | – (logické px; drží střed, clamp na monitor) |
| `delete_api_key` | – | – (smaže klíč z trezoru) |
| `get_autostart` / `set_autostart` | – / `{enabled}` | bool / – (tauri-plugin-autostart, HKCU Run) |
| `default_config` | – | `Config` (výchozí hodnoty) |
| `list_ollama_models` | `{url}` | `string[]` (GET /api/tags) |
| `test_prompt` | `{config, prompt, text}` | string (celý výsledek, pro editor akce) |
| `list_models` | – | `[{id, label, provider}]` (Ollama tags + Claude, když je klíč) |
| `get_default_model` / `set_default_model` | – / `{spec}` | spec / – (spec = `ollama:<m>` \| `anthropic:<id>`, uloží config) |

## Eventy (Rust → TS)

- `popup-open` `{ text, actions[], auto_action? }` – při zobrazení popupu
- `llm-token` `{ seq, token, done, error? }` – streamované kousky odpovědi

## Design rozhodnutí

- **Clipboard trik místo Accessibility API** – funguje ve všech appkách bez integrace,
  cena = krátké přepsání schránky (zálohuje se a obnovuje). Stejný přístup má většina
  podobných nástrojů.
- **Popup bere fokus** (kvůli klávesovému ovládání) → před vložením musíme původní okno
  refokusovat (`focus_window`). Proto se HWND ukládá hned při hotkey.
- **`release_modifiers()` před Ctrl+C/V** – uživatel při hotkey ještě drží Ctrl+Shift;
  bez uvolnění by cílová appka dostala Ctrl+Shift+C.
- **Popup se schová při ztrátě fokusu** (`WindowEvent::Focused(false)`), Esc taky.
- **Appka žije v tray**, `ExitRequested` bez kódu se blokuje, ukončení jen z tray menu.
- **Thinking modely (Gemma 4, Qwen 3)**: ai-hotkey posílá `think: false` (`/api/chat`); pro OpenAI klienty (Leo) slouží bridge, protože `/v1` endpoint Ollamy `think` neumí, jen `reasoning_effort`.
- **Streaming** – oba providery streamují, UI vykresluje průběžně; u `replace` akcí se
  vloží až kompletní text (nechceme vkládat po tokenech).

## Známé limity / TODO

- Windows `SetForegroundWindow` může selhat, když jiná appka právě drží fokus „násilím";
  fallback = text zůstane ve schránce, uživatel dá Ctrl+V ručně.
- Appky, které blokují Ctrl+C na výběru (RDP, některé terminály), výběr nedodají → popup
  ukáže „Nic není označeno".
- Každá akce může mít vlastní globální zkratku (`Action.hotkey`), registruje `register_hotkeys()`.
- Hotkey editor v Nastavení je textové pole – formát viz
  [tauri-plugin-global-shortcut](https://v2.tauri.app/plugin/global-shortcut/) (`Ctrl+Shift+Space`,
  `Alt+Q`, `Super+K`…).
- Autostart přes `tauri-plugin-autostart` (přepínač v Nastavení → Obecné).
- Ikona: `src-tauri/icons/app-icon.svg` → `npx tauri icon src-tauri/icons/app-icon.svg`.
