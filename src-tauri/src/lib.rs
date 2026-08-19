//! ai-hotkey – vstupní bod Tauri aplikace.
//!
//! Tok: globální hotkey → [`platform::grab_selection`] → popup okno u kurzoru
//! → uživatel vybere akci → [`llm`] streamuje výsledek do UI (event `llm-token`)
//! → „Nahradit" = [`platform::paste_text`] do původního okna, „Kopírovat" = do schránky.

mod actions;
mod bridge;
mod config;
mod llm;
mod platform;
mod secrets;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use config::Config;
use platform::{CurrentPlatform, Platform, WindowHandle};

const POPUP_LABEL: &str = "popup";
const SETTINGS_LABEL: &str = "settings";
const POPUP_W: f64 = 320.0;
const POPUP_H: f64 = 320.0;

/// Sdílený stav aplikace.
pub struct AppState {
    config: Mutex<Config>,
    platform: Arc<dyn Platform>,
    /// Okno, které mělo fokus při stisku hotkey (kam se bude vkládat výsledek).
    target: Mutex<Option<WindowHandle>>,
    /// Text sebraný při hotkey.
    selection: Mutex<String>,
    /// Číslo běžícího LLM požadavku – starší streamy se ignorují (uživatel klikl znovu).
    request_seq: Mutex<u64>,
    /// Vlastní evidence viditelnosti popupu (nevolat `is_visible()` z event callbacků).
    popup_visible: AtomicBool,
    /// Připnutý panel se při ztrátě fokusu nezavírá (uživatel ho odsunul a čte originál).
    popup_pinned: AtomicBool,
}

#[derive(Serialize, Clone)]
struct PopupPayload {
    text: String,
    actions: Vec<actions::Action>,
    /// Když je vyplněno, frontend akci spustí rovnou (přímá zkratka akce, bez kolečka).
    auto_action: Option<String>,
    auto_replace: bool,
    output_language: String,
}

#[derive(Serialize, Clone)]
struct TokenPayload {
    seq: u64,
    token: String,
    done: bool,
    error: Option<String>,
}

// ---------- Tauri commandy (volané z frontendu) ----------

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(app: AppHandle, state: tauri::State<'_, AppState>, mut config: Config) -> Result<(), String> {
    let old = state.config.lock().unwrap().clone();
    // API klíč: frontend pošle neprázdný jen když uživatel zadal nový → do trezoru;
    // prázdný = ponechat stávající (z paměti). Do config.json se klíč nikdy nezapíše.
    if !config.anthropic.api_key.is_empty() {
        secrets::set(secrets::ANTHROPIC_KEY, &config.anthropic.api_key)?;
    } else {
        config.anthropic.api_key = old.anthropic.api_key.clone();
    }
    config.anthropic.api_key_stored = !config.anthropic.api_key.is_empty();
    config.save().map_err(|e| e.to_string())?;
    register_hotkeys(&app, Some(&old), &config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = config;
    Ok(())
}

/// Smaže Anthropic API klíč z trezoru OS.
#[tauri::command]
fn delete_api_key(state: tauri::State<'_, AppState>) -> Result<(), String> {
    secrets::delete(secrets::ANTHROPIC_KEY)?;
    let mut cfg = state.config.lock().unwrap();
    cfg.anthropic.api_key.clear();
    cfg.anthropic.api_key_stored = false;
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn default_config() -> Config {
    Config::default()
}

/// Seznam modelů na Ollama serveru (pro rozbalovací seznam v Nastavení).
#[tauri::command]
async fn list_ollama_models(url: String) -> Result<Vec<String>, String> {
    let u = format!("{}/api/tags", url.trim_end_matches('/'));
    let v: serde_json::Value = reqwest::Client::new()
        .get(&u)
        .send()
        .await
        .map_err(|e| format!("Ollama nedostupná: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v["models"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
        .unwrap_or_default())
}

/// Vyzkoušet prompt na vzorovém textu (editor akce). Vrací celý výsledek najednou.
#[tauri::command]
async fn test_prompt(state: tauri::State<'_, AppState>, mut config: Config, prompt: String, text: String) -> Result<String, String> {
    if config.anthropic.api_key.is_empty() {
        config.anthropic.api_key = state.config.lock().unwrap().anthropic.api_key.clone();
    }
    let system = prompt.replace("{lang}", &config.target_language).replace("{out}", &config.output_language);
    let provider = llm::from_config(&config);
    let sink = |_: &str| {};
    provider.complete(&system, &text, &sink).await.map_err(|e| e.to_string())
}

/// Zajistí, že Ollama běží: když neodpovídá, spustí `ollama app.exe` (Windows) a počká až ~20 s.
/// Vrací true, když je Ollama dostupná.
#[tauri::command]
async fn ensure_ollama(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let url = state.config.lock().unwrap().ollama.url.trim_end_matches('/').to_string();
    Ok(ensure_ollama_inner(&url).await)
}

async fn ensure_ollama_inner(url: &str) -> bool {
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(2)).build().unwrap();
    let ping = |http: reqwest::Client, url: String| async move { http.get(format!("{url}/api/tags")).send().await.map(|r| r.status().is_success()).unwrap_or(false) };
    if ping(http.clone(), url.to_string()).await {
        return true;
    }
    if !url.contains("localhost") && !url.contains("127.0.0.1") {
        return false; // vzdálená Ollama – nespouštíme
    }
    log::warn!("Ollama neodpovídá, zkouším ji spustit…");
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let candidates = [
            format!("{base}\\Programs\\Ollama\\ollama app.exe"),
            format!("{base}\\Programs\\Ollama\\ollama.exe"),
        ];
        for c in candidates {
            if std::path::Path::new(&c).exists() {
                let mut cmd = std::process::Command::new(&c);
                if c.ends_with("ollama.exe") {
                    cmd.arg("serve");
                }
                match cmd.spawn() {
                    Ok(_) => { log::info!("spuštěno {c}"); break; }
                    Err(e) => log::warn!("spuštění {c} selhalo: {e}"),
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("ollama").arg("serve").spawn();
    }
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if ping(http.clone(), url.to_string()).await {
            log::info!("Ollama běží");
            return true;
        }
    }
    false
}

#[derive(Serialize, Clone)]
struct ModelInfo {
    id: String,
    label: String,
    provider: String,
}

/// Modely pro rychlé přepínání: Ollama (živě ze serveru) + Claude (když je klíč).
#[tauri::command]
async fn list_models(state: tauri::State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let cfg = state.config.lock().unwrap().clone();
    let mut out = Vec::new();
    if let Ok(models) = list_ollama_models(cfg.ollama.url.clone()).await {
        for m in models {
            out.push(ModelInfo { id: format!("ollama:{m}"), label: format!("{m} · Ollama"), provider: "ollama".into() });
        }
    }
    if !cfg.anthropic.api_key.is_empty() {
        for (id, label) in llm::ANTHROPIC_MODELS {
            out.push(ModelInfo { id: format!("anthropic:{id}"), label: (*label).into(), provider: "anthropic".into() });
        }
    }
    Ok(out)
}

#[tauri::command]
fn get_default_model(state: tauri::State<'_, AppState>) -> String {
    llm::default_spec(&state.config.lock().unwrap())
}

/// Nastaví výchozí model (provider + název) a uloží config.
#[tauri::command]
fn set_default_model(state: tauri::State<'_, AppState>, spec: String) -> Result<(), String> {
    let mut cfg = state.config.lock().unwrap();
    if let Some(m) = spec.strip_prefix("anthropic:") {
        cfg.provider = "anthropic".into();
        cfg.anthropic.model = m.to_string();
    } else if let Some(m) = spec.strip_prefix("ollama:") {
        cfg.provider = "ollama".into();
        cfg.ollama.model = m.to_string();
    } else {
        return Err(format!("neplatný model: {spec}"));
    }
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_provider(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let cfg = state.config.lock().unwrap().clone();
    llm::from_config(&cfg).health().await.map_err(|e| e.to_string())
}

/// Spustí akci nad aktuálně sebraným textem; výsledek streamuje eventem `llm-token`.
#[tauri::command]
async fn run_action(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    action_id: String,
    text: Option<String>,
    model: Option<String>,
) -> Result<u64, String> {
    let cfg = state.config.lock().unwrap().clone();
    let action = cfg
        .actions
        .iter()
        .find(|a| a.id == action_id)
        .cloned()
        .ok_or_else(|| format!("neznámá akce {action_id}"))?;
    let input = text.unwrap_or_else(|| state.selection.lock().unwrap().clone());
    let seq = {
        let mut s = state.request_seq.lock().unwrap();
        *s += 1;
        *s
    };
    let system = action.render_prompt(&cfg.target_language, &cfg.output_language);
    // Priorita: explicitní volba v panelu > model akce > výchozí model
    let spec = model
        .filter(|m| !m.is_empty())
        .or_else(|| (!action.model.is_empty()).then(|| action.model.clone()))
        .unwrap_or_else(|| llm::default_spec(&cfg));
    log::info!("run_action {} → {}", action.id, spec);
    let provider = llm::from_spec(&cfg, &spec);
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let app3 = app2.clone();
        let sink = move |t: &str| {
            let _ = app3.emit(
                "llm-token",
                TokenPayload { seq, token: t.to_string(), done: false, error: None },
            );
        };
        let res = provider.complete(&system, &input, &sink).await;
        let payload = match res {
            Ok(_) => TokenPayload { seq, token: String::new(), done: true, error: None },
            Err(e) => TokenPayload { seq, token: String::new(), done: true, error: Some(e.to_string()) },
        };
        let _ = app2.emit("llm-token", payload);
    });
    Ok(seq)
}

/// „Nahradit": schová popup a vloží text do původního okna.
#[tauri::command]
fn paste_result(app: AppHandle, state: tauri::State<'_, AppState>, text: String) -> Result<(), String> {
    hide_popup(&app);
    let target = *state.target.lock().unwrap();
    log::info!("paste_result → {target:?}, {} znaků", text.chars().count());
    let p = state.platform.clone();
    std::thread::spawn(move || {
        if let Err(e) = platform::paste_text(p.as_ref(), target, &text) {
            log::error!("paste selhal: {e}");
        }
    });
    Ok(())
}

#[tauri::command]
fn copy_result(app: AppHandle, text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text).map_err(|e| e.to_string())?;
    let _ = app; // popup zavírá frontend po krátkém „✓ Zkopírováno"
    Ok(())
}

/// Změní velikost popupu (logické px) a drží ho vycentrovaný na stejném středu + v monitoru.
/// Používá frontend při přechodu kolečko → výsledkový panel (výška podle obsahu).
#[tauri::command]
fn resize_popup(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let Some(w) = app.get_webview_window(POPUP_LABEL) else { return Ok(()) };
    let scale = w.scale_factor().unwrap_or(1.0);
    let (nw, nh) = (width * scale, height * scale);
    let pos = w.outer_position().map_err(|e| e.to_string())?;
    let size = w.outer_size().map_err(|e| e.to_string())?;
    let (cx, cy) = (pos.x as f64 + size.width as f64 / 2.0, pos.y as f64 + size.height as f64 / 2.0);
    let (mut x, mut y) = (cx - nw / 2.0, cy - nh / 2.0);
    if let Ok(Some(mon)) = app.monitor_from_point(cx, cy) {
        let mp = mon.position();
        let ms = mon.size();
        let (left, top) = (mp.x as f64, mp.y as f64);
        let (right, bottom) = (left + ms.width as f64, top + ms.height as f64);
        x = x.max(left).min(right - nw);
        y = y.max(top).min(bottom - nh);
    }
    w.set_size(tauri::PhysicalSize::new(nw, nh)).map_err(|e| e.to_string())?;
    w.set_position(PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Autostart při přihlášení (HKCU\...\Run) – zapíná/vypíná uživatel v Nastavení.
#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())
}

#[tauri::command]
fn set_popup_pinned(state: tauri::State<'_, AppState>, pinned: bool) {
    state.popup_pinned.store(pinned, Ordering::SeqCst);
}

#[tauri::command]
fn close_popup(app: AppHandle) {
    hide_popup(&app);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings(&app);
}

// ---------- Okna ----------

fn hide_popup(app: &AppHandle) {
    app.state::<AppState>().popup_visible.store(false, Ordering::SeqCst);
    app.state::<AppState>().popup_pinned.store(false, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window(POPUP_LABEL) {
        let _ = w.hide();
    }
}

fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("index.html#settings".into()))
        .title("ai-hotkey – Nastavení")
        .inner_size(880.0, 650.0)
        .min_inner_size(720.0, 520.0)
        .build();
}

/// Ukáže popup u kurzoru (drží se v hranicích monitoru) a pošle mu text + akce.
fn show_popup(app: &AppHandle, text: String, auto_action: Option<String>) {
    let cfg = app.state::<AppState>().config.lock().unwrap().clone();
    let Some(w) = app.get_webview_window(POPUP_LABEL) else { return };

    // Kolečko se centruje na kurzor; drží se v hranicích monitoru.
    if let Ok(pos) = app.cursor_position() {
        let scale = w.scale_factor().unwrap_or(1.0);
        let (pw, ph) = (POPUP_W * scale, POPUP_H * scale);
        let _ = w.set_size(tauri::PhysicalSize::new(pw, ph));
        let (mut x, mut y) = (pos.x - pw / 2.0, pos.y - ph / 2.0);
        if let Ok(Some(mon)) = app.monitor_from_point(pos.x, pos.y) {
            let mp = mon.position();
            let ms = mon.size();
            let (left, top) = (mp.x as f64, mp.y as f64);
            let (right, bottom) = (left + ms.width as f64, top + ms.height as f64);
            x = x.max(left).min(right - pw);
            y = y.max(top).min(bottom - ph);
        }
        log::info!("popup @ {x},{y} (cursor {},{})", pos.x, pos.y);
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let actions: Vec<actions::Action> = cfg.actions.into_iter().filter(|a| a.enabled).collect();
    let _ = w.emit("popup-open", PopupPayload { text, actions, auto_action, auto_replace: cfg.auto_replace, output_language: cfg.output_language });
    app.state::<AppState>().popup_visible.store(true, Ordering::SeqCst);
    let _ = w.show();
    let _ = w.set_focus();
}

/// Reakce na hotkey: zapamatovat cílové okno, sebrat výběr, ukázat popup.
fn on_hotkey(app: &AppHandle, auto_action: Option<String>) {
    // POZOR: tohle běží uvnitř callbacku event loopu. Volat tady synchronně metody okna
    // (is_visible/hide/show) může na Windows způsobit deadlock → všechno děláme
    // v samostatném vlákně a viditelnost popupu si držíme sami v AppState.
    let app2 = app.clone();
    std::thread::spawn(move || {
        let state = app2.state::<AppState>();
        if state.popup_visible.load(Ordering::SeqCst) && auto_action.is_none() {
            hide_popup(&app2);
            return;
        }
        let fg = state.platform.foreground_window();
        log::info!("hotkey: foreground={fg:?}");
        *state.target.lock().unwrap() = fg;
        let p = state.platform.clone();
        let text = match platform::grab_selection(p.as_ref()) {
            Ok(Some(t)) => t,
            Ok(None) => String::new(),
            Err(e) => {
                log::error!("grab_selection: {e}");
                String::new()
            }
        };
        log::info!("hotkey: selection {} znaků", text.chars().count());
        *app2.state::<AppState>().selection.lock().unwrap() = text.clone();
        let app3 = app2.clone();
        let _ = app2.run_on_main_thread(move || show_popup(&app3, text, auto_action));
    });
}

/// Zaregistruje hlavní zkratku (kolečko) + přímé zkratky jednotlivých akcí.
/// `old` = předchozí config, jehož zkratky se nejdřív odregistrují.
fn register_hotkeys(app: &AppHandle, old: Option<&Config>, cfg: &Config) -> anyhow::Result<()> {
    let gs = app.global_shortcut();
    if let Some(o) = old {
        let mut olds = vec![o.hotkey.clone()];
        olds.extend(o.actions.iter().map(|a| a.hotkey.clone()));
        for h in olds.iter().filter(|h| !h.is_empty()) {
            if let Ok(s) = h.parse::<Shortcut>() {
                let _ = gs.unregister(s);
            }
        }
    }
    let main: Shortcut = cfg
        .hotkey
        .parse()
        .map_err(|e| anyhow::anyhow!("neplatná zkratka '{}': {e}", cfg.hotkey))?;
    gs.on_shortcut(main, |app, _sc, ev| {
        if ev.state == ShortcutState::Pressed {
            on_hotkey(app, None);
        }
    })?;
    log::info!("hotkey {} zaregistrována", cfg.hotkey);
    for a in cfg.actions.iter().filter(|a| !a.hotkey.is_empty()) {
        let sc: Shortcut = match a.hotkey.parse() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("akce {}: neplatná zkratka '{}': {e}", a.id, a.hotkey);
                continue;
            }
        };
        let id = a.id.clone();
        if let Err(e) = gs.on_shortcut(sc, move |app, _sc, ev| {
            if ev.state == ShortcutState::Pressed {
                on_hotkey(app, Some(id.clone()));
            }
        }) {
            log::warn!("akce {}: zkratku '{}' nejde zaregistrovat: {e}", a.id, a.hotkey);
        } else {
            log::info!("akce {} → {}", a.id, a.hotkey);
        }
    }
    Ok(())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Nastavení", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Ukončit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &quit])?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("ai-hotkey")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| match ev.id.as_ref() {
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let config = Config::load();
    let platform: Arc<dyn Platform> = Arc::new(CurrentPlatform::new().expect("platform init"));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(AppState {
            config: Mutex::new(config),
            platform,
            target: Mutex::new(None),
            selection: Mutex::new(String::new()),
            request_seq: Mutex::new(0),
            popup_visible: AtomicBool::new(false),
            popup_pinned: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            check_provider,
            run_action,
            paste_result,
            copy_result,
            close_popup,
            resize_popup,
            get_autostart,
            delete_api_key,
            default_config,
            list_ollama_models,
            list_models,
            ensure_ollama,
            set_popup_pinned,
            get_default_model,
            set_default_model,
            test_prompt,
            set_autostart,
            open_settings
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;
            let cfg = handle.state::<AppState>().config.lock().unwrap().clone();
            if let Err(e) = register_hotkeys(&handle, None, &cfg) {
                log::error!("{e}");
            }
            if cfg.leo_bridge.enabled {
                bridge::start(cfg.ollama.url.clone(), cfg.leo_bridge.port);
            }
            if cfg.provider == "ollama" {
                let url = cfg.ollama.url.clone();
                tauri::async_runtime::spawn(async move { ensure_ollama_inner(&url).await; });
            }
            // Popup: při ztrátě fokusu schovat
            if let Some(w) = app.get_webview_window(POPUP_LABEL) {
                let h = handle.clone();
                w.on_window_event(move |ev| {
                    if let tauri::WindowEvent::Focused(false) = ev {
                        if h.state::<AppState>().popup_pinned.load(Ordering::SeqCst) {
                            return;
                        }
                        // hide() mimo callback (viz on_hotkey). Krátce počkat: při zahájení tažení
                        // za hlavičku okno na moment ztratí fokus a hned ho dostane zpět – to není
                        // „klik jinam", takže nezavírat.
                        let h2 = h.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(250));
                            let st = h2.state::<AppState>();
                            if st.popup_pinned.load(Ordering::SeqCst) || !st.popup_visible.load(Ordering::SeqCst) {
                                return;
                            }
                            if let Some(w) = h2.get_webview_window(POPUP_LABEL) {
                                if w.is_focused().unwrap_or(false) {
                                    return;
                                }
                            }
                            hide_popup(&h2);
                        });
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, ev| {
            // Zavření posledního okna nesmí ukončit appku – žije v tray.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = ev {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
