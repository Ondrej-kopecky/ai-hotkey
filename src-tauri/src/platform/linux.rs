//! Linux implementace [`Platform`] – ZATÍM NEIMPLEMENTOVÁNO (viz docs/LINUX-PORT.md).
//!
//! Plán:
//! - X11: fokus přes `x11rb` (`_NET_ACTIVE_WINDOW`), klávesy přes enigo (xdo backend).
//! - Wayland: fokus nelze programově vracet; klávesy přes `ydotool`/`wtype`, nebo enigo
//!   s `libei` backendem. Fallback = jen zkopírovat výsledek do schránky.

use super::{Platform, WindowHandle};
use anyhow::{bail, Result};

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Platform for LinuxPlatform {
    fn foreground_window(&self) -> Option<WindowHandle> {
        None
    }
    fn focus_window(&self, _w: WindowHandle) -> Result<()> {
        bail!("Linux: focus_window není implementováno")
    }
    fn send_copy(&self) -> Result<()> {
        bail!("Linux: send_copy není implementováno")
    }
    fn send_paste(&self) -> Result<()> {
        bail!("Linux: send_paste není implementováno")
    }
    fn release_modifiers(&self) -> Result<()> {
        Ok(())
    }
}
