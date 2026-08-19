# Linux port – brief pro agenta

Cíl: zprovoznit ai-hotkey na Linuxu **beze změny** frontendu, LLM vrstvy ani `lib.rs`.
Veškerá práce je v `src-tauri/src/platform/linux.rs` + build závislosti + ověření
chování Tauri pluginů na X11/Wayland.

Nejdřív si přečti `docs/ARCHITECTURE.md` a `src-tauri/src/platform/mod.rs` (trait
`Platform` a helpery `grab_selection` / `paste_text`, které Linux implementaci volají).

## 0. Zjisti prostředí (první krok, ovlivňuje všechno další)

```bash
echo $XDG_SESSION_TYPE        # x11 | wayland
echo $XDG_CURRENT_DESKTOP     # GNOME | KDE | ...
```

## 1. Build závislosti (Tauri v2 na Linuxu)

Debian/Ubuntu:
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```
Fedora: `webkit2gtk4.1-devel openssl-devel libxdo-devel libappindicator-gtk3-devel librsvg2-devel`.
Arch: `webkit2gtk-4.1 base-devel xdotool openssl libappindicator-gtk3 librsvg`.

`libxdo-dev` je pro enigo (X11 backend). Tray na Linuxu = `libayatana-appindicator`.

## 2. Implementuj `platform/linux.rs`

Trait `Platform`:

| Metoda | X11 | Wayland |
|---|---|---|
| `foreground_window()` | `x11rb`: přečíst `_NET_ACTIVE_WINDOW` z root okna → `WindowHandle(xid)` | vrať `None` (kompozitor okna nevydá) |
| `focus_window(w)` | `x11rb`: poslat `_NET_ACTIVE_WINDOW` ClientMessage na root, nebo `xdotool windowactivate` | `Ok(())` no-op – po skrytí popupu se fokus zpravidla vrátí sám |
| `send_copy()` / `send_paste()` | enigo (xdo backend) `Ctrl+C` / `Ctrl+V` – jako Windows | `wtype -M ctrl c -m ctrl` nebo `ydotool key 29:1 46:1 46:0 29:0` (vyžaduje ydotoold + práva k /dev/uinput); enigo má experimentální `libei` feature |
| `release_modifiers()` | enigo release Ctrl/Shift/Alt/Meta | totéž přes wtype/ydotool, jinak no-op |

Doporučení: udělej `LinuxPlatform` s enum backendem `{X11, WaylandWtype, WaylandYdotool, None}`
zvoleným v `new()` podle `XDG_SESSION_TYPE` a dostupnosti binárek (`which wtype`), a loguj,
který se vybral. Když nic není k dispozici, `send_copy` vrátí `Err` → popup ukáže „Nic není
označeno" a `paste_text` selže → uživatel má výsledek aspoň ve schránce (copy_result funguje
vždy přes arboard).

Přidej do `Cargo.toml`:
```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = "0.13"
```

### Alternativa pro výběr textu na X11
Na X11 existuje **PRIMARY selection** = aktuálně označený text bez nutnosti Ctrl+C:
`arboard::Clipboard::get().clipboard(LinuxClipboardKind::Primary).text()`. To je lepší než
clipboard trik – nemění uživatelovu schránku a nepotřebuje simulovat klávesy. Zvaž
přepsání `grab_selection` tak, aby na Linuxu/X11 nejdřív zkusil PRIMARY a na Ctrl+C
spadl jen jako fallback (bude třeba přidat do traitu volitelnou metodu
`fn read_selection_direct(&self) -> Option<String> { None }`).

## 3. Ověř Tauri vrstvu na Linuxu

- **Global shortcut**: `tauri-plugin-global-shortcut` na Waylandu obecně **nefunguje**
  (kompozitor nepovolí globální grab). Fallbacky: (a) běžet přes XWayland, (b) na GNOME/KDE
  nechat uživatele nastavit systémovou zkratku, která spustí `ai-hotkey --trigger` – druhá
  instance pošle signál běžící (Unix socket / `tauri-plugin-single-instance` s argv), nebo
  (c) `xdg-desktop-portal` GlobalShortcuts API (portal v1.1+, KDE ho má, GNOME 46+).
  Doporučený MVP: (b) přes `tauri-plugin-single-instance` – jednoduché a spolehlivé.
- **Popup okno**: `transparent: true` vyžaduje kompozitor (běžně OK); `alwaysOnTop` a
  `skipTaskbar` na Waylandu částečně ignorované – není kritické.
- **`app.cursor_position()`** na Waylandu může vrátit chybu → v `show_popup` už je to
  ošetřené (`if let Ok(pos)`), popup se jen neposune. Případně vycentrovat na obrazovku.
- **Tray**: potřebuje appindicator; na GNOME rozšíření AppIndicator.
- **`WindowEvent::Focused(false)`** pro auto-hide popupu – ověř, že chodí i na Waylandu.

## 4. Test plán (ručně)

1. `npm run tauri dev` – appka nastartuje, ikona v tray, log „hotkey … zaregistrována"
   (nebo očekávaná chyba na Waylandu + fallback).
2. V gedit/Kate označ text, hotkey → popup s náhledem textu.
3. Akce „Shrnout" → streamuje výsledek, Ctrl+C → v schránce.
4. Akce „Opravit gramatiku" (replace) → text v editoru se nahradí, schránka je po chvíli
   obnovena na původní obsah.
5. Totéž ve Firefoxu (textarea) a v terminálu (tam se čeká, že Ctrl+C výběr nedodá – popup
   má napsat „Nic není označeno").
6. Nastavení: změna hotkey → funguje bez restartu.

## 5. Balení

`npm run tauri build` → `.deb` / `.AppImage` / `.rpm` podle `bundle.targets` (nyní jen `nsis`
pro Windows – přidej `"deb", "appimage"` nebo nastav `"all"`).

## Co NEdělat

- Nesahat do `lib.rs`, `llm/`, `main.ts` kvůli platformním rozdílům – pokud něco Linux
  potřebuje navíc, rozšiř trait `Platform` výchozí implementací (default method), aby
  Windows zůstal beze změny.
- Nepřidávat Linux-only crates bez `[target.'cfg(target_os = "linux")']`.
