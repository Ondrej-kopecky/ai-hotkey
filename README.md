# ai-hotkey

Označ text v libovolné aplikaci, stiskni **Ctrl+Shift+Space** a vyber si v kruhovém menu, co s ním
udělat: **opravit gramatiku, přeložit, shrnout, upravit styl, vysvětlit** – nebo si přidej vlastní
akce. Výsledek nahradí označený text, nebo si ho zkopíruješ.

Běží **lokálně a zdarma** přes [Ollama](https://ollama.com) (model Gemma 4). Volitelně můžeš u
libovolné akce přepnout na Claude (Anthropic API), když chceš lepší kvalitu.

- Windows 10/11 (hotové) · Linux (v plánu, viz [docs/LINUX-PORT.md](docs/LINUX-PORT.md))
- Inspirace: wheelly.ai

## Instalace (uživatel)

1. **Nainstaluj Ollamu** – [ollama.com/download](https://ollama.com/download) (Windows installer,
   po instalaci běží na pozadí a startuje s Windows).
2. **Stáhni model** – otevři PowerShell/Terminál a spusť:
   ```
   ollama pull gemma4:12b
   ```
   (~7,6 GB; potřebuje grafiku s ~10 GB VRAM. Na slabším stroji použij `gemma4:e4b` – menší a
   rychlejší, o něco horší čeština – a v Nastavení appky pak vyber tento model.)
3. **Nainstaluj ai-hotkey** – z [Releases](../../releases) stáhni `ai-hotkey_x.y.z_x64-setup.exe`
   a spusť. Windows SmartScreen může varovat (aplikace není podepsaná certifikátem) → *Další
   informace → Přesto spustit*.
4. Aplikace se schová do **oznamovací oblasti** (ikona u hodin). Označ jakýkoli text a stiskni
   **Ctrl+Shift+Space**.

Tip: pravým tlačítkem na ikonu → **Nastavení** → zapni *Spouštět po startu Windows*.

## Ovládání

**Kruhové menu** – po stisknutí zkratky u kurzoru:

| | |
|---|---|
| myš / šipky / písmeno akce (`G` `P` `S` `T` `V`) / `1`–`9` | výběr akce |
| `Enter` | spustit vybranou akci |
| `Esc`, klik na střed, klik mimo | zavřít |

**Panel s výsledkem:**

| | |
|---|---|
| `Enter` | hlavní akce – *Nahradit výběr* (gramatika, styl) nebo *Kopírovat* (překlad, shrnutí, vysvětlení) |
| `Ctrl+Enter` / `Ctrl+C` | druhá možnost (nahradit / kopírovat) |
| `Tab` | přepnout Výsledek ↔ Porovnání (u nahrazovacích akcí) |
| `Backspace` | zpět do menu · `Esc` zavřít |
| tažení za hlavičku | přesunout panel · ikona špendlíku = nezavírat při kliknutí jinam |
| výběr modelu vpravo nahoře | přepočítat jiným modelem (a zapamatovat jako výchozí) |

## Nastavení (ikona → Nastavení)

- **Obecné** – zkratka pro menu, jazyk odpovědí, výchozí jazyk překladu, autostart, okamžité nahrazování.
- **AI poskytovatel** – Ollama (adresa, model ze seznamu, test připojení) nebo Anthropic (API klíč,
  model `claude-sonnet-5`). Když je klíč vyplněný, Claude modely se objeví ve výběru v panelu i u akcí.
- **Akce** – karty s přepínačem, pořadí přetažením (= pořadí v kolečku), editor: název, ikona,
  písmeno, vlastní globální zkratka, režim *Náhled* / *Nahradit výběr*, pevný model, prompt
  s proměnnými `{out}` (jazyk odpovědí) a `{lang}` (jazyk překladu), tlačítko *Vyzkoušet*.

Config je JSON v `%APPDATA%\ai-hotkey\config.json`. **API klíč se do něj neukládá** – je v šifrovaném
trezoru systému (Windows Správce pověření / macOS Keychain / Linux Secret Service), vázaný na tvůj
uživatelský účet.

## Brave Leo (bonus)

Appka spouští lokální most `http://localhost:11435/v1/chat/completions` (vypnutelný v Nastavení),
který přeposílá požadavky do Ollamy a vypíná „thinking" modelu (Gemma 4 jinak přemýšlí minuty).
V Brave: `brave://settings/leo` → *Bring your own model* → endpoint na port **11435**, model
`gemma4:12b`, klíč prázdný.

## Soukromí

S Ollamou nic neopouští tvůj počítač. Při přepnutí na Claude se označený text posílá do Anthropic
API. Aplikace nemá žádnou telemetrii.

## Vývoj

Požadavky: Rust (stable), Node 18+, na Windows WebView2 (ve Win11 je) a MSVC build tools.

```bash
npm install
npm run tauri dev       # dev build s hot-reloadem frontendu
npm run tauri build     # release → src-tauri/target/release/bundle/nsis/*-setup.exe
```

Release na GitHubu: pushni tag `vX.Y.Z` → workflow `.github/workflows/release.yml` sestaví
instalátor a připne ho k Release.

Struktura a rozhodnutí: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Licence MIT.
