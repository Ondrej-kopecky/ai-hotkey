//! Ukládání tajemství (API klíčů) do trezoru operačního systému přes crate `keyring`:
//! Windows → Správce pověření (DPAPI), Linux → Secret Service (GNOME Keyring/KWallet),
//! macOS → Keychain. Do `config.json` se klíče nikdy nezapisují.

use keyring::Entry;

const SERVICE: &str = "ai-hotkey";

fn entry(name: &str) -> Option<Entry> {
    Entry::new(SERVICE, name)
        .map_err(|e| log::warn!("keyring entry {name}: {e}"))
        .ok()
}

/// Vrátí uložené tajemství, nebo None (neexistuje / trezor nedostupný).
pub fn get(name: &str) -> Option<String> {
    let e = entry(name)?;
    match e.get_password() {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) => None,
        Err(keyring::Error::NoEntry) => None,
        Err(err) => {
            log::warn!("keyring get {name}: {err}");
            None
        }
    }
}

pub fn set(name: &str, value: &str) -> Result<(), String> {
    let e = entry(name).ok_or("trezor OS není dostupný")?;
    e.set_password(value).map_err(|err| format!("uložení do trezoru selhalo: {err}"))
}

pub fn delete(name: &str) -> Result<(), String> {
    let Some(e) = entry(name) else { return Ok(()) };
    match e.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!("smazání z trezoru selhalo: {err}")),
    }
}

/// Název položky pro Anthropic API klíč.
pub const ANTHROPIC_KEY: &str = "anthropic-api-key";
