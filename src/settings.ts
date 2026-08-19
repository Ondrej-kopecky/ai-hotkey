// Nastavení – levá navigace + sekce, karty akcí, editor akce v dialogu.
import { invoke } from "@tauri-apps/api/core";
import { ICONS, esc, type Action, type Config, type ModelInfo } from "./shared";

type Section = "general" | "provider" | "actions" | "about";
const NAV: { id: Section; label: string; icon: string }[] = [
  { id: "general", label: "Obecné", icon: '<circle cx="10" cy="10" r="3"/><path d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4"/>' },
  { id: "provider", label: "AI poskytovatel", icon: '<rect x="3" y="4" width="14" height="12" rx="2"/><path d="M6.5 8.5h7M6.5 11.5h4"/>' },
  { id: "actions", label: "Akce", icon: '<circle cx="10" cy="10" r="7"/><path d="M10 3v14M3 10h14"/>' },
  { id: "about", label: "O aplikaci", icon: '<circle cx="10" cy="10" r="7.5"/><path d="M10 9v5M10 6.5v.5"/>' },
];
const ACTION_ICONS = ["check", "translate", "list", "pen", "bulb", "spark"];
const SAMPLE = "Ahoj, tohle je vjeta s chybama, kterou bysme mjeli opravit, aby vypadala profesionálně.";
const ico = (n: string, cls = "") => `<svg class="ic ${cls}" viewBox="0 0 20 20" width="18" height="18">${ICONS[n] ?? ICONS.spark}</svg>`;
const deepClone = <T,>(o: T): T => JSON.parse(JSON.stringify(o));

export async function settings(root: HTMLElement) {
  let saved = await invoke<Config>("get_config");
  let cfg = deepClone(saved);
  let section: Section = "general";
  let status = "";
  let autostart = false;
  try { autostart = await invoke<boolean>("get_autostart"); } catch { /* plugin nedostupný */ }
  let ollamaModels: string[] = [];
  let ollamaState: "unknown" | "ok" | "err" = "unknown";
  let ollamaMsg = "";
  let showKey = false;
  let editKey = false;
  let dragFrom = -1;
  let models: ModelInfo[] = [];
  invoke<ModelInfo[]>("list_models").then((m) => { models = m; }).catch(() => {});
  const dirty = () => JSON.stringify(cfg) !== JSON.stringify(saved);

  async function refreshModels(silent = false) {
    try {
      ollamaModels = await invoke<string[]>("list_ollama_models", { url: cfg.ollama.url });
      ollamaState = "ok";
      ollamaMsg = ollamaModels.includes(cfg.ollama.model) ? "Připojeno" : `Připojeno · model ${cfg.ollama.model} není stažený`;
    } catch (e) {
      ollamaState = "err"; ollamaMsg = String(e); ollamaModels = [];
    }
    if (!silent) render();
  }
  refreshModels(true).then(render);

  // ---------- render ----------
  function render() {
    root.innerHTML = `
      <div class="st">
        <nav class="st-nav">
          <div class="st-brand"><img src="/icon.png" alt="" width="22" height="22"> ai-hotkey</div>
          ${NAV.map((n) => `<button class="${section === n.id ? "on" : ""}" data-s="${n.id}">${ico("", "")}${n.label}</button>`.replace(ico("", ""), `<svg class="ic" viewBox="0 0 20 20" width="18" height="18">${n.icon}</svg>`)).join("")}
        </nav>
        <main class="st-main">
          <div class="st-content">${SECTIONS[section]()}</div>
          <footer class="st-foot">
            <button class="ghost" id="reset">Obnovit výchozí</button>
            <span class="status ${status.startsWith("Chyba") ? "error" : ""}">${esc(status)}</span>
            <span class="spacer"></span>
            <button id="cancel" ${dirty() ? "" : "disabled"}>Zrušit</button>
            <button class="primary" id="save" ${dirty() ? "" : "disabled"}>Uložit změny</button>
          </footer>
        </main>
        <div id="dialog"></div>
      </div>`;
    bind();
  }

  const field = (label: string, control: string, hint = "") =>
    `<label class="f"><span class="fl">${label}</span>${control}${hint ? `<span class="fh">${hint}</span>` : ""}</label>`;
  const toggle = (id: string, on: boolean, label: string, hint: string) =>
    `<div class="tg"><label class="sw"><input type="checkbox" id="${id}" ${on ? "checked" : ""}><span></span></label><div><div class="tl">${label}</div><div class="fh">${hint}</div></div></div>`;

  const SECTIONS: Record<Section, () => string> = {
    general: () => `
      <h2>Obecné</h2>
      <section class="card">
        ${field("Otevřít kruhové menu", `<input id="hotkey" value="${esc(cfg.hotkey)}" placeholder="Ctrl+Shift+Space" data-capture>`, "Klikni do pole a stiskni kombinaci kláves.")}
        <div class="grid2">
          ${field("Jazyk odpovědí", `<input id="out" value="${esc(cfg.output_language)}" placeholder="Czech">`, "Používá se v akcích jako {out}.")}
          ${field("Výchozí jazyk překladu", `<input id="lang" value="${esc(cfg.target_language)}" placeholder="English">`, "Používá se v akcích jako {lang}.")}
        </div>
      </section>
      <section class="card">
        ${toggle("autostart", autostart, "Spouštět po startu Windows", "Přidá ai-hotkey do „Po spuštění“ (HKCU Run). Projeví se ihned.")}
        ${toggle("auto_replace", cfg.auto_replace, "Okamžitě nahradit výběr", "Výsledek akcí v režimu „nahradit“ se vloží ihned bez zobrazení náhledu.")}
      </section>`,

    provider: () => `
      <h2>AI poskytovatel</h2>
      <div class="tabs">
        <button class="${cfg.provider === "ollama" ? "on" : ""}" data-p="ollama">Ollama</button>
        <button class="${cfg.provider === "anthropic" ? "on" : ""}" data-p="anthropic">Anthropic</button>
        <button disabled title="Připravuje se">OpenAI</button>
      </div>
      ${cfg.provider === "ollama" ? `
      <section class="card">
        ${field("Adresa serveru", `<input id="ollama_url" value="${esc(cfg.ollama.url)}">`)}
        <label class="f"><span class="fl">Model</span>
          <div class="row">
            <select id="ollama_model">
              ${(ollamaModels.includes(cfg.ollama.model) ? ollamaModels : [cfg.ollama.model, ...ollamaModels]).map((m) => `<option ${m === cfg.ollama.model ? "selected" : ""}>${esc(m)}</option>`).join("")}
            </select>
            <button id="refresh_models" title="Načíst modely ze serveru">Obnovit modely</button>
          </div>
        </label>
        <div class="row between">
          <span class="dot ${ollamaState}">● ${esc(ollamaState === "unknown" ? "Zjišťuji…" : ollamaMsg)}</span>
          <button id="check">Otestovat připojení</button>
        </div>
      </section>
      <section class="card">
        ${toggle("bridge", cfg.leo_bridge.enabled, "Most pro Brave Leo (a jiné OpenAI klienty)", `Lokální proxy na <code>http://localhost:${cfg.leo_bridge.port}/v1/chat/completions</code>, která přeposílá požadavky do Ollamy a vypíná „thinking“ (jinak Gemma 4 přemýšlí minuty). Do Lea zadej tento endpoint místo portu 11434. Změna vyžaduje restart aplikace.`)}
        ${field("Port", `<input id="bridge_port" type="number" value="${cfg.leo_bridge.port}" min="1024" max="65535" style="max-width:140px">`)}
      </section>` : `
      <section class="card">
        <div class="f"><span class="fl">API klíč</span>
          ${cfg.anthropic.api_key_stored && !editKey ? `
          <div class="row between keyrow">
            <span class="keymask">••••••••••••</span>
            <span class="fh keyinfo">uloženo v trezoru systému</span>
            <span class="spacer"></span>
            <button type="button" id="key_change">Změnit</button>
            <button type="button" id="key_delete" class="ghost">Smazat</button>
          </div>
          <span class="fh">Windows Správce pověření / macOS Keychain / Linux Secret Service – šifrováno na tvůj účet, v config.json není.</span>` : `
          <div class="row">
            <input id="anth_key" type="${showKey ? "text" : "password"}" value="${esc(cfg.anthropic.api_key)}" placeholder="sk-ant-…" autocomplete="off">
            <button type="button" class="icon" id="eye" title="${showKey ? "Skrýt" : "Zobrazit"}">${showKey ? "🙈" : "👁"}</button>
            ${cfg.anthropic.api_key_stored ? `<button type="button" id="key_cancel" class="ghost">Zrušit</button>` : ""}
          </div>
          <span class="fh">Klíč se uloží do šifrovaného trezoru systému, ne do config.json. Po zadání klikni na Uložit změny.</span>`}
        </div>
        ${field("Model", `<input id="anth_model" value="${esc(cfg.anthropic.model)}" list="anth_models" placeholder="claude-sonnet-5"><datalist id="anth_models"><option value="claude-sonnet-5"><option value="claude-opus-5"><option value="claude-haiku-4-5"></datalist>`, "Doporučeno claude-sonnet-5. Klíč vlož a ulož – pak se Claude modely objeví ve výběru v panelu i u akcí.")}
        <div class="row between"><span class="fh">Data se posílají do cloudu Anthropic.</span><button id="check">Otestovat připojení</button></div>
      </section>`}`,

    actions: () => `
      <div class="hrow"><h2>Akce v kruhovém menu</h2><button class="primary" id="add">+ Přidat akci</button></div>
      <p class="fh">Pořadí karet = pořadí v kolečku (přetáhni). Vypnuté akce se v kolečku nezobrazují.</p>
      <div class="cards">
        ${cfg.actions.map((a, i) => `
          <div class="acard ${a.enabled ? "" : "off"}" draggable="true" data-i="${i}">
            <span class="grip" title="Přetáhnout">⋮⋮</span>
            ${ico(a.icon, "big")}
            <div class="ainfo">
              <div class="aname">${esc(a.name)} ${a.key ? `<kbd>${esc(a.key.toUpperCase())}</kbd>` : ""} ${a.hotkey ? `<kbd>${esc(a.hotkey)}</kbd>` : ""}${a.model ? `<kbd title="Model">${esc(models.find((m) => m.id === a.model)?.label ?? a.model)}</kbd>` : ""}</div>
              <div class="fh">${esc(a.description || (a.mode === "replace" ? "Náhled a nahrazení výběru" : "Zobrazí výsledek"))}</div>
            </div>
            <label class="sw" title="Zapnuto"><input type="checkbox" class="a-on" data-i="${i}" ${a.enabled ? "checked" : ""}><span></span></label>
            <button class="a-edit" data-i="${i}">Upravit</button>
            <button class="icon a-del" data-i="${i}" title="Smazat">🗑</button>
          </div>`).join("")}
      </div>`,

    about: () => `
      <h2>O aplikaci</h2>
      <section class="card about">
        <img src="/icon.png" width="56" height="56" alt="">
        <div>
          <div class="aname">ai-hotkey <span class="fh">0.1.1</span></div>
          <div class="fh">Označ text kdekoli, stiskni zkratku, nech AI pracovat. Běží lokálně (Ollama) nebo přes Anthropic API.</div>
          <div class="fh" style="margin-top:8px">Config: <code>%APPDATA%\\ai-hotkey\\config.json</code></div>
        </div>
      </section>`,
  };

  // ---------- bind ----------
  const $ = <T extends HTMLElement>(sel: string) => root.querySelector<T>(sel);
  function bind() {
    root.querySelectorAll<HTMLButtonElement>(".st-nav button").forEach((b) => b.onclick = () => { section = b.dataset.s as Section; render(); });
    $("#save")!.onclick = save;
    $("#cancel")!.onclick = () => { cfg = deepClone(saved); status = ""; render(); };
    $("#reset")!.onclick = async () => {
      if (!confirm("Obnovit výchozí nastavení? Vlastní akce a klíče se ztratí (do uložení jde vrátit tlačítkem Zrušit).")) return;
      cfg = await invoke<Config>("default_config"); status = "Výchozí hodnoty načteny – ulož změny."; render();
    };
    // Obecné
    $("#hotkey")?.addEventListener("keydown", captureHotkey((v) => { cfg.hotkey = v; render(); }));
    $("#out")?.addEventListener("input", (e) => cfg.output_language = (e.target as HTMLInputElement).value);
    $("#lang")?.addEventListener("input", (e) => cfg.target_language = (e.target as HTMLInputElement).value);
    $("#autostart")?.addEventListener("change", async (e) => {
      const on = (e.target as HTMLInputElement).checked;
      try { await invoke("set_autostart", { enabled: on }); autostart = on; status = on ? "Autostart zapnut" : "Autostart vypnut"; }
      catch (err) { status = "Chyba autostartu: " + err; }
      render();
    });
    $("#auto_replace")?.addEventListener("change", (e) => { cfg.auto_replace = (e.target as HTMLInputElement).checked; render(); });
    // Poskytovatel
    root.querySelectorAll<HTMLButtonElement>(".tabs button[data-p]").forEach((b) => b.onclick = () => { cfg.provider = b.dataset.p as Config["provider"]; render(); });
    $("#ollama_url")?.addEventListener("change", (e) => { cfg.ollama.url = (e.target as HTMLInputElement).value.trim(); refreshModels(); });
    $("#ollama_model")?.addEventListener("change", (e) => { cfg.ollama.model = (e.target as HTMLSelectElement).value; render(); });
    $("#refresh_models")?.addEventListener("click", () => refreshModels());
    $("#bridge")?.addEventListener("change", (e) => { cfg.leo_bridge.enabled = (e.target as HTMLInputElement).checked; render(); });
    $("#bridge_port")?.addEventListener("change", (e) => { cfg.leo_bridge.port = +(e.target as HTMLInputElement).value || 11435; render(); });
    $("#anth_key")?.addEventListener("input", (e) => cfg.anthropic.api_key = (e.target as HTMLInputElement).value.trim());
    $("#anth_model")?.addEventListener("input", (e) => cfg.anthropic.model = (e.target as HTMLInputElement).value.trim());
    $("#eye")?.addEventListener("click", () => { showKey = !showKey; render(); });
    $("#key_change")?.addEventListener("click", () => { editKey = true; render(); });
    $("#key_cancel")?.addEventListener("click", () => { editKey = false; cfg.anthropic.api_key = ""; render(); });
    $("#key_delete")?.addEventListener("click", async () => {
      if (!confirm("Smazat Anthropic API klíč z trezoru systému?")) return;
      try { await invoke("delete_api_key"); cfg.anthropic.api_key = ""; cfg.anthropic.api_key_stored = false; saved = deepClone(cfg); status = "Klíč smazán"; }
      catch (e) { status = "Chyba: " + e; }
      render();
    });
    $("#check")?.addEventListener("click", async () => {
      try { await invoke("save_config", { config: cfg }); cfg = await invoke<Config>("get_config"); saved = deepClone(cfg); editKey = false; status = await invoke<string>("check_provider"); }
      catch (e) { status = "Chyba: " + e; }
      render();
    });
    // Akce
    $("#add")?.addEventListener("click", () => openEditor(-1));
    root.querySelectorAll<HTMLButtonElement>(".a-edit").forEach((b) => b.onclick = () => openEditor(+b.dataset.i!));
    root.querySelectorAll<HTMLButtonElement>(".a-del").forEach((b) => b.onclick = () => { cfg.actions.splice(+b.dataset.i!, 1); render(); });
    root.querySelectorAll<HTMLInputElement>(".a-on").forEach((c) => c.onchange = () => { cfg.actions[+c.dataset.i!].enabled = c.checked; render(); });
    root.querySelectorAll<HTMLElement>(".acard").forEach((card) => {
      card.ondragstart = (e) => { dragFrom = +card.dataset.i!; e.dataTransfer!.effectAllowed = "move"; card.classList.add("dragging"); };
      card.ondragover = (e) => { e.preventDefault(); card.classList.add("over"); };
      card.ondragleave = () => card.classList.remove("over");
      card.ondrop = (e) => {
        e.preventDefault();
        const to = +card.dataset.i!;
        if (dragFrom >= 0 && dragFrom !== to) { const [m] = cfg.actions.splice(dragFrom, 1); cfg.actions.splice(to, 0, m); }
        dragFrom = -1; render();
      };
      card.ondragend = () => { dragFrom = -1; render(); };
    });
  }

  async function save() {
    try { await invoke("save_config", { config: cfg }); cfg = await invoke<Config>("get_config"); saved = deepClone(cfg); editKey = false; status = "Uloženo"; }
    catch (e) { status = "Chyba: " + e; }
    render();
  }

  // Zachytí stisknutou kombinaci a zapíše ji ve formátu pluginu (Ctrl+Shift+G).
  function captureHotkey(set: (v: string) => void) {
    return (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Backspace" || e.key === "Delete") { set(""); return; }
      if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
      const mods = [e.ctrlKey && "Ctrl", e.shiftKey && "Shift", e.altKey && "Alt", e.metaKey && "Super"].filter(Boolean) as string[];
      let k = e.key;
      if (k === " ") k = "Space";
      else if (k.length === 1) k = k.toUpperCase();
      set([...mods, k].join("+"));
    };
  }

  // ---------- editor akce ----------
  function openEditor(index: number) {
    const a: Action = index >= 0 ? deepClone(cfg.actions[index]) : { id: "custom" + Date.now().toString(36), name: "Nová akce", icon: "spark", prompt: "", mode: "show", key: "", hotkey: "", enabled: true, description: "", model: "" };
    const dlg = root.querySelector<HTMLElement>("#dialog")!;
    let testing = false, testOut = "";
    const draw = () => {
      dlg.innerHTML = `
        <div class="overlay">
          <div class="modal">
            <div class="mhead"><h3>${index >= 0 ? "Upravit akci" : "Nová akce"}</h3><button class="icon" id="m-close">×</button></div>
            <div class="mbody">
              <div class="grid2">
                ${field("Název", `<input id="m-name" value="${esc(a.name)}" placeholder="Např. Přeložit do němčiny">`)}
                ${field("Písmeno v kolečku", `<input id="m-key" value="${esc(a.key)}" maxlength="1" placeholder="např. N">`, "Jedna klávesa pro rychlý výběr.")}
              </div>
              ${field("Popis", `<input id="m-desc" value="${esc(a.description)}" placeholder="Krátce, co akce dělá (zobrazí se na kartě)">`)}
              <div class="grid2">
                <div class="f"><span class="fl">Ikona</span>
                  <div class="icons">${ACTION_ICONS.map((n) => `<button type="button" class="ipick ${a.icon === n ? "on" : ""}" data-ic="${n}" title="${n}">${ico(n)}</button>`).join("")}</div>
                </div>
                <div class="f"><span class="fl">Režim výsledku</span>
                  <div class="seg2"><button type="button" class="${a.mode === "show" ? "on" : ""}" data-m="show">Náhled</button><button type="button" class="${a.mode === "replace" ? "on" : ""}" data-m="replace">Nahradit výběr</button></div>
                  <span class="fh">${a.mode === "replace" ? "Po potvrzení nahradí označený text; nabídne Porovnání změn." : "Zobrazí výsledek ke zkopírování (překlad, shrnutí…)."}</span>
                </div>
              </div>
              <div class="grid2">
                ${field("Globální zkratka", `<input id="m-hotkey" value="${esc(a.hotkey)}" placeholder="klikni a stiskni kombinaci…" data-capture>`, "Volitelné – spustí akci rovnou bez kolečka. Backspace smaže.")}
                ${field("Model", `<select id="m-model"><option value="">Výchozí</option>${models.map((m) => `<option value="${esc(m.id)}" ${a.model === m.id ? "selected" : ""}>${esc(m.label)}</option>`).join("")}</select>`, "Výchozí = model z Nastavení / volby v panelu.")}
              </div>
              <label class="f"><span class="fl row">Prompt <span class="fh">· vložit proměnnou:</span> <button type="button" class="chip" data-var="{out}">{out}</button> <button type="button" class="chip" data-var="{lang}">{lang}</button></span>
                <textarea id="m-prompt" rows="6" placeholder="Instrukce pro model. Označený text jde jako zpráva uživatele.">${esc(a.prompt)}</textarea>
              </label>
              <div class="row between">
                <button id="m-test" ${testing ? "disabled" : ""}>${testing ? "Zkouším…" : "Vyzkoušet na vzorovém textu"}</button>
                <span class="fh">„${esc(SAMPLE.slice(0, 48))}…"</span>
              </div>
              ${testOut ? `<pre class="testout">${esc(testOut)}</pre>` : ""}
            </div>
            <div class="mfoot"><button id="m-cancel">Zrušit</button><button class="primary" id="m-ok">${index >= 0 ? "Použít" : "Přidat"}</button></div>
          </div>
        </div>`;
      const q = <T extends HTMLElement>(s: string) => dlg.querySelector<T>(s)!;
      const close = () => { dlg.innerHTML = ""; };
      q("#m-close").onclick = close; q("#m-cancel").onclick = close;
      q<HTMLInputElement>("#m-name").oninput = (e) => a.name = (e.target as HTMLInputElement).value;
      q<HTMLInputElement>("#m-key").oninput = (e) => a.key = (e.target as HTMLInputElement).value.slice(0, 1);
      q<HTMLInputElement>("#m-desc").oninput = (e) => a.description = (e.target as HTMLInputElement).value;
      q<HTMLTextAreaElement>("#m-prompt").oninput = (e) => a.prompt = (e.target as HTMLTextAreaElement).value;
      q<HTMLSelectElement>("#m-model").onchange = (e) => a.model = (e.target as HTMLSelectElement).value;
      q("#m-hotkey").addEventListener("keydown", captureHotkey((v) => { a.hotkey = v; draw(); }));
      dlg.querySelectorAll<HTMLButtonElement>(".ipick").forEach((b) => b.onclick = () => { a.icon = b.dataset.ic!; draw(); });
      dlg.querySelectorAll<HTMLButtonElement>(".seg2 button").forEach((b) => b.onclick = () => { a.mode = b.dataset.m as Action["mode"]; draw(); });
      dlg.querySelectorAll<HTMLButtonElement>(".chip").forEach((b) => b.onclick = () => {
        const ta = q<HTMLTextAreaElement>("#m-prompt"); const v = b.dataset.var!;
        const s0 = ta.selectionStart, s1 = ta.selectionEnd;
        ta.value = ta.value.slice(0, s0) + v + ta.value.slice(s1); a.prompt = ta.value; ta.focus(); ta.selectionStart = ta.selectionEnd = s0 + v.length;
      });
      q("#m-test").onclick = async () => {
        testing = true; testOut = ""; draw();
        try {
          const c = deepClone(cfg);
          if (a.model.startsWith("anthropic:")) { c.provider = "anthropic"; c.anthropic.model = a.model.slice(10); }
          else if (a.model.startsWith("ollama:")) { c.provider = "ollama"; c.ollama.model = a.model.slice(7); }
          testOut = await invoke<string>("test_prompt", { config: c, prompt: a.prompt, text: SAMPLE });
        }
        catch (e) { testOut = "Chyba: " + e; }
        testing = false; draw();
      };
      q("#m-ok").onclick = () => {
        if (!a.name.trim()) { q<HTMLInputElement>("#m-name").focus(); return; }
        if (!a.id.trim()) a.id = "custom" + Date.now().toString(36);
        if (index >= 0) cfg.actions[index] = a; else cfg.actions.push(a);
        close(); render();
      };
      dlg.querySelector<HTMLElement>(".overlay")!.onclick = (e) => { if (e.target === e.currentTarget) close(); };
    };
    draw();
  }

  render();
}
