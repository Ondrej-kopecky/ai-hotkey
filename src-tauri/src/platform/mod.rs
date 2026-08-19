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
    // Některé appky (Teams) dávají do textu místo emoji jejich slovní název; v HTML verzi
    // schránky je ale skutečný znak → opravit podle HTML.
    if let Some(t) = grabbed.as_mut() {
        if let Ok(html) = cb.get().html() {
            *t = fix_emoji_from_html(t, &html);
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

/// Teams kopíruje emoji jako `<readonly … itemtype="http://schema.skype.com/Emoji" itemscope="😄"
/// title="Zubící se tvář…">`, zatímco plain text obsahuje jen ten název. Tady se názvy v textu
/// nahradí znakem z HTML. Jiné HTML se ignoruje.
pub fn fix_emoji_from_html(text: &str, html: &str) -> String {
    if !html.contains("schema.skype.com/Emoji") {
        return text.to_string();
    }
    // element s atributy v libovolném pořadí: vytáhnout itemscope (znak) a title/alt (název)
    let re_el = regex::Regex::new(r#"<readonly[^>]*schema\.skype\.com/Emoji[^>]*>"#).unwrap();
    let re_scope = regex::Regex::new(r#"itemscope="([^"]+)""#).unwrap();
    let re_title = regex::Regex::new(r#"(?:title|aria-label)="([^"]+)""#).unwrap();
    let mut out = text.to_string();
    let mut seen: Vec<(String, String)> = Vec::new();
    for el in re_el.find_iter(html) {
        let el = el.as_str();
        let Some(emoji) = re_scope.captures(el).and_then(|c| c.get(1)).map(|m| m.as_str()) else { continue };
        let Some(name) = re_title.captures(el).and_then(|c| c.get(1)).map(|m| m.as_str()) else { continue };
        let name = html_unescape(name);
        if emoji.is_empty() || name.is_empty() || seen.iter().any(|(n, _)| n == &name) {
            continue;
        }
        seen.push((name, emoji.to_string()));
    }
    // delší názvy první, aby se nepřepsal kus delšího názvu kratším
    seen.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (name, emoji) in seen {
        // Teams dává v plain textu emoji na samostatný řádek → zalomení před názvem nahradit mezerou,
        // emoji je v HTML inline. Zalomení za emoji nechat (může to být konec odstavce).
        let re_name = regex::Regex::new(&format!(r"[ \t]*(?:\r?\n)+[ \t]*{}", regex::escape(&name))).unwrap();
        out = re_name.replace_all(&out, format!(" {emoji}").as_str()).to_string();
        out = out.replace(&name, &emoji);
    }
    out
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::fix_emoji_from_html;

    #[test]
    fn teams_emoji_name_is_replaced() {
        let text = "Věděli jsem, že to nebude sslehké. Zubící se tvář se smějícíma se očima";
        let html = r#"<!--StartFragment-->Věděli jsem, že to nebude sslehké. <readonly contenteditable="false" title="Zubící se tvář se smějícíma se očima" itemid="grinningfacewithsmilingeyes" itemtype="http://schema.skype.com/Emoji" itemscope="😄" aria-label="Zubící se tvář se smějícíma se očima"><img alt="Zubící se tvář se smějícíma se očima"></readonly><!--EndFragment-->"#;
        assert_eq!(fix_emoji_from_html(text, html), "Věděli jsem, že to nebude sslehké. 😄");
        // Teams: emoji v plain textu na novém řádku → přilepit za text
        let text2 = "Věděli jsem, že to nebude sslehké.

Zubící se tvář se smějícíma se očima
Další věta.";
        assert_eq!(fix_emoji_from_html(text2, html), "Věděli jsem, že to nebude sslehké. 😄
Další věta.");
    }

    #[test]
    fn other_html_untouched() {
        assert_eq!(fix_emoji_from_html("ahoj", "<b>ahoj</b>"), "ahoj");
    }
}
