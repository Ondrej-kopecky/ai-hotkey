//! Windows implementace [`Platform`] – Win32 pro fokus oken, enigo pro klávesy.

use super::{Platform, WindowHandle};
use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::Mutex;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

pub struct WindowsPlatform {
    enigo: Mutex<Enigo>,
}

impl WindowsPlatform {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow!("enigo init: {e:?}"))?;
        Ok(Self { enigo: Mutex::new(enigo) })
    }

    fn combo(&self, modifier: Key, key: Key) -> Result<()> {
        let mut e = self.enigo.lock().unwrap();
        e.key(modifier, Direction::Press).map_err(|e| anyhow!("{e:?}"))?;
        e.key(key, Direction::Click).map_err(|e| anyhow!("{e:?}"))?;
        e.key(modifier, Direction::Release).map_err(|e| anyhow!("{e:?}"))?;
        Ok(())
    }
}

impl Platform for WindowsPlatform {
    fn foreground_window(&self) -> Option<WindowHandle> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            None
        } else {
            Some(WindowHandle(hwnd.0 as isize))
        }
    }

    fn focus_window(&self, w: WindowHandle) -> Result<()> {
        let hwnd = HWND(w.0 as *mut _);
        // SetForegroundWindow může selhat, pokud náš proces není "foreground-eligible";
        // Tauri okno ale právě mělo fokus, takže to zpravidla projde.
        let ok = unsafe { SetForegroundWindow(hwnd) };
        if ok.as_bool() {
            Ok(())
        } else {
            Err(anyhow!("SetForegroundWindow selhalo"))
        }
    }

    fn send_copy(&self) -> Result<()> {
        self.combo(Key::Control, Key::Unicode('c'))
    }

    fn send_paste(&self) -> Result<()> {
        self.combo(Key::Control, Key::Unicode('v'))
    }

    fn release_modifiers(&self) -> Result<()> {
        let mut e = self.enigo.lock().unwrap();
        for k in [Key::Control, Key::Shift, Key::Alt, Key::Meta] {
            let _ = e.key(k, Direction::Release);
        }
        Ok(())
    }
}
