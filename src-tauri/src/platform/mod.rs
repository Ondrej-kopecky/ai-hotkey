//! Platformní vrstva – JEDINÉ místo s OS-specifickým kódem.
//!
//! Pro port na Linux stačí implementovat trait [`Platform`] v `linux.rs`
//! (viz docs/LINUX-PORT.md) a nic jiného v aplikaci se měnit nemusí.

use anyhow::Result;
use std::time::Duration;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::WindowsPlatform as CurrentPlatform;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use self::linux::LinuxPlatform as CurrentPlatform;

/// Neprůhledný identifikátor okna, které mělo fokus v momentě stisknutí hotkey.
/// Windows: HWND jako isize. Linux/X11: XID. Wayland: nemá smysl (nelze refokusovat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowHandle(pub isize);

pub trait Platform: Send + Sync {
    /// Vrátí aktuálně aktivní (foreground) okno – volá se hned při hotkey,
    /// ještě než se ukáže náš popup.
    fn foreground_window(&self) -> Option<WindowHandle>;

    /// Aktivuje dané okno (vrátí mu fokus). Použije se před vložením výsledku.
    fn focus_window(&self, w: WindowHandle) -> Result<()>;

    /// Pošle do aktivního okna Ctrl+C (na macOS by to bylo Cmd+C).
    fn send_copy(&self) -> Result<()>;

    /// Pošle do aktivního okna Ctrl+V.
    fn send_paste(&self) -> Result<()>;

    /// Uvolní modifikátory (Ctrl/Shift/Alt), které uživatel může ještě držet
    /// z hotkey – jinak by se Ctrl+C poslalo jako Ctrl+Shift+C.
    fn release_modifiers(&self) -> Result<()>;
}

/// Sebere aktuálně označený text: uloží schránku, pošle Ctrl+C, přečte schránku, obnoví ji.
/// Vrací `Ok(None)`, když nic nebylo označeno (schránka se nezměnila / je prázdná).
pub fn grab_selection(p: &dyn Platform) -> Result<Option<String>> {
    let mut cb = arboard::Clipboard::new()?;
    let backup = cb.get_text().ok();

    // Vyprázdnit, ať poznáme, že Ctrl+C něco skutečně dodalo.
    let _ = cb.set_text(String::new());
    p.release_modifiers()?;
    std::thread::sleep(Duration::from_millis(60));
    p.send_copy()?;

    // Počkat, až cílová appka schránku naplní (některé jsou pomalé – Electron, Office).
    let mut grabbed: Option<String> = None;
    for _ in 0..15 {
        std::thread::sleep(Duration::from_millis(30));
        if let Ok(t) = cb.get_text() {
            if !t.is_empty() {
                grabbed = Some(t);
                break;
            }
        }
    }

    // Obnovit původní schránku (uživatel si ji nechce nechat přepsat).
    if let Some(b) = backup {
        let _ = cb.set_text(b);
    }
    Ok(grabbed)
}

/// Vloží text místo výběru: nastaví schránku, refokusuje původní okno, pošle Ctrl+V,
/// po chvíli obnoví schránku.
pub fn paste_text(p: &dyn Platform, target: Option<WindowHandle>, text: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new()?;
    let backup = cb.get_text().ok();
    cb.set_text(text.to_string())?;

    if let Some(w) = target {
        p.focus_window(w)?;
        std::thread::sleep(Duration::from_millis(120));
    }
    p.release_modifiers()?;
    p.send_paste()?;

    // Dát cílové appce čas si schránku přečíst, pak vrátit původní obsah.
    std::thread::sleep(Duration::from_millis(300));
    if let Some(b) = backup {
        let _ = cb.set_text(b);
    }
    Ok(())
}
