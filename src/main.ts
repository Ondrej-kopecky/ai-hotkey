// Frontend ai-hotkey – jeden bundle, dvě obrazovky podle URL hashe:
//   #popup    – radiální kolečko u kurzoru → výsledkový panel (stream, Porovnání, Nahradit/Kopírovat)
//   #settings – nastavení (hotkey, provider, model, akce)
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ICONS, iconSvg, esc, type Action, type ModelInfo, type PopupPayload, type TokenPayload } from "./shared";
import { settings } from "./settings";
import { mdToHtml, mdToPlain } from "./markdown";

const app = document.getElementById("app")!;

// Titulek výsledkového panelu podle typu akce
const RESULT_TITLE: Record<string, string> = { check: "Opravený text", pen: "Upravený text", translate: "Překlad", list: "Shrnutí", bulb: "Vysvětlení" };
const LANG_CS: Record<string, string> = { czech: "čeština", english: "angličtina", german: "němčina", slovak: "slovenština", polish: "polština", french: "francouzština", spanish: "španělština" };

// ---- slovní diff (LCS) pro režim Porovnání ----
type DiffOp = { t: "eq" | "del" | "ins"; s: string };
function wordDiff(a: string, b: string): DiffOp[] {
  const A = a.split(/(\s+)/).filter((x) => x !== ""), B = b.split(/(\s+)/).filter((x) => x !== "");
  const n = A.length, m = B.length;
  if (n * m > 4_000_000) return [{ t: "del", s: a }, { t: "ins", s: b }]; // pojistka na obří texty
  const dp: Uint16Array[] = Array.from({ length: n + 1 }, () => new Uint16Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--)
    dp[i][j] = A[i] === B[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const out: DiffOp[] = [];
  const push = (t: DiffOp["t"], s: string) => { const l = out[out.length - 1]; if (l && l.t === t) l.s += s; else out.push({ t, s }); };
  let i = 0, j = 0;
  while (i < n && j < m) {
    if (A[i] === B[j]) { push("eq", A[i]); i++; j++; }
    else if (dp[i + 1][j] >= dp[i][j + 1]) { push("del", A[i]); i++; }
    else { push("ins", B[j]); j++; }
  }
  while (i < n) push("del", A[i++]);
  while (j < m) push("ins", B[j++]);
  return out;
}

// =====================================================================
// POPUP – kolečko + výsledek
// =====================================================================
function popup() {
  const SIZE = 320;          // logická velikost plátna kolečka (= POPUP_W/H v lib.rs)
  const R = 138;             // vnější poloměr kolečka
  const R_IN = 40;           // poloměr středu (Zavřít)
  const C = SIZE / 2;
  const PANEL_W = 560;       // šířka výsledkového panelu

  let text = "";
  let actions: Action[] = [];
  let outputLang = "";
  let selected = -1;
  let centerHover = false;
  let stage: "pick" | "result" = "pick";
  let action: Action | null = null;
  let seq = 0;
  let result = "";
  let running = false;
  let error: string | null = null;
  let autoReplace = false;
  let view: "result" | "diff" = "result";
  let copied = false;
  let pinned = false;
  const uiIcon = (n: string) => `<svg class="ui" viewBox="0 0 20 20" width="16" height="16">${ICONS[n]}</svg>`;
  let models: ModelInfo[] = [];
  let defaultModel = "";     // spec výchozího modelu ("ollama:…" / "anthropic:…")
  let usedModel = "";        // model použitý pro aktuální výsledek
  const modelLabel = (spec: string) => models.find((m) => m.id === spec)?.label ?? spec.replace(/^\w+:/, "");
  async function loadModels() {
    try { models = await invoke<ModelInfo[]>("list_models"); defaultModel = await invoke<string>("get_default_model"); } catch { /* ignore */ }
  }

  const polar = (r: number, aDeg: number) => {
    const a = ((aDeg - 90) * Math.PI) / 180;
    return [C + r * Math.cos(a), C + r * Math.sin(a)];
  };
  const sectorPath = (a0: number, a1: number, r = R, rin = R_IN) => {
    const [x0, y0] = polar(r, a0), [x1, y1] = polar(r, a1);
    const [i0, j0] = polar(rin, a0), [i1, j1] = polar(rin, a1);
    const large = a1 - a0 > 180 ? 1 : 0;
    return `M${x0},${y0} A${r},${r} 0 ${large} 1 ${x1},${y1} L${i1},${j1} A${rin},${rin} 0 ${large} 0 ${i0},${j0} Z`;
  };

  // ---------------- kolečko ----------------
  function renderWheel() {
    const n = actions.length;
    const step = n ? 360 / n : 360;
    const disabled = !text;
    const sectors = actions.map((a, i) => {
      const a0 = i * step - step / 2, a1 = a0 + step;
      const mid = a0 + step / 2;
      const [cx, cy] = polar((R + R_IN) / 2 + 6, mid);
      const sel = i === selected && !disabled;
      const lines = wrapLabel(a.name);
      const lineH = 16;
      const blockH = 24 + lines.length * lineH + (a.key ? 15 : 0);
      let y = cy - blockH / 2 + 10;
      let inner = iconSvg(a.icon, cx, y); y += 24;
      for (const l of lines) { inner += `<text x="${cx}" y="${y}" class="lbl">${esc(l)}</text>`; y += lineH; }
      if (a.key) inner += `<g transform="translate(${cx},${y - 3})"><rect x="-9" y="-9" width="18" height="14" rx="3" class="key"/><text x="0" y="2" class="keyt">${esc(a.key.toUpperCase())}</text></g>`;
      return `
        <g class="sector ${sel ? "sel" : ""} ${disabled ? "dis" : ""}" data-i="${i}" style="transform-origin:${cx}px ${cy}px">
          <path d="${sectorPath(a0, a1)}" class="fill"/>
          ${inner}
        </g>`;
    }).join("");
    const seps = actions.map((_, i) => {
      const a = i * step - step / 2;
      const [x0, y0] = polar(R_IN, a), [x1, y1] = polar(R, a);
      return `<line x1="${x0}" y1="${y0}" x2="${x1}" y2="${y1}" class="sep"/>`;
    }).join("");
    const centerLabel = disabled
      ? `<text x="${C}" y="${C - 3}" class="ctr err">Nic není</text><text x="${C}" y="${C + 11}" class="ctr err">označeno</text>`
      : `<text x="${C}" y="${C + 1}" class="ctr x ${centerHover ? "hov" : ""}">×</text><text x="${C}" y="${C + 16}" class="ctr small">Zavřít</text>`;

    app.innerHTML = `
      <div class="wheel">
        <svg viewBox="0 0 ${SIZE} ${SIZE}" width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
          <circle cx="${C}" cy="${C}" r="${R}" class="bg"/>
          ${sectors}
          ${seps}
          <circle cx="${C}" cy="${C}" r="${R}" class="ring"/>
          <circle id="center" cx="${C}" cy="${C}" r="${R_IN - 3}" class="center ${centerHover ? "hov" : ""}"/>
          ${centerLabel}
        </svg>
      </div>`;

    const svg = app.querySelector("svg")!;
    svg.addEventListener("mousemove", (e) => {
      const { i, center } = hit(e as MouseEvent, svg);
      if (i !== selected || center !== centerHover) { selected = i; centerHover = center; renderWheel(); }
    });
    svg.addEventListener("mouseleave", () => { if (selected !== -1 || centerHover) { selected = -1; centerHover = false; renderWheel(); } });
    svg.addEventListener("click", (e) => {
      const { i } = hit(e as MouseEvent, svg);
      if (i >= 0) run(i);
      else invoke("close_popup");
    });
  }

  function wrapLabel(name: string): string[] {
    const words = name.split(" ");
    const lines: string[] = [];
    let cur = "";
    for (const w of words) {
      if ((cur + " " + w).trim().length > 12 && cur) { lines.push(cur); cur = w; } else cur = (cur + " " + w).trim();
    }
    if (cur) lines.push(cur);
    const l = lines.slice(0, 2);
    if (lines.length > 2) l[1] = l[1].slice(0, 11) + "…";
    return l;
  }

  function hit(e: MouseEvent, svg: SVGSVGElement): { i: number; center: boolean } {
    const rect = svg.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * SIZE - C;
    const y = ((e.clientY - rect.top) / rect.height) * SIZE - C;
    const d = Math.hypot(x, y);
    if (d < R_IN) return { i: -1, center: true };
    if (d > R || !actions.length) return { i: -1, center: false };
    let ang = (Math.atan2(y, x) * 180) / Math.PI + 90;
    const step = 360 / actions.length;
    ang = (ang + step / 2 + 360) % 360;
    return { i: Math.floor(ang / step), center: false };
  }

  // Lidská nápověda k nejčastějším chybám (chybí Ollama / model / klíč)
  function errorHint(e: string): string {
    const l = e.toLowerCase();
    let h = "";
    if (l.includes("nedostupn") || l.includes("connection") || l.includes("connect")) h = "Ollama neběží nebo není nainstalovaná. Nainstaluj ji z <b>ollama.com/download</b>, pak stáhni model příkazem <code>ollama pull gemma4:12b</code>. Appka se ji pokusí spustit sama při dalším otevření menu.";
    else if (l.includes("not found") || l.includes("nenalezen") || l.includes("model")) h = "Model není stažený. V terminálu spusť <code>ollama pull gemma4:12b</code> (nebo v Nastavení → AI poskytovatel vyber jiný model ze seznamu).";
    else if (l.includes("api klíč") || l.includes("api key") || l.includes("401") || l.includes("authentication")) h = "Anthropic API klíč chybí nebo je neplatný – Nastavení → AI poskytovatel → Anthropic.";
    return h ? `<div class="hint">${h}</div>` : "";
  }

  // ---------------- výsledkový panel ----------------
  function renderResult() {
    const title = RESULT_TITLE[action?.icon ?? ""] ?? "Výsledek";
    const lang = LANG_CS[outputLang.toLowerCase()] ?? outputLang;
    const isReplace = action?.mode === "replace";
    const canDiff = isReplace && !!text;
    const body = error
      ? `<div class="error">${esc(error)}</div>${errorHint(error)}`
      : view === "diff" && canDiff && !running
        ? `<div class="result diff" id="result">${wordDiff(text, result).map((o) => `<span class="${o.t}">${esc(o.s)}</span>`).join("")}</div>`
        : isReplace
          ? `<div class="result" id="result">${esc(result)}</div>`
          : `<div class="result md" id="result">${mdToHtml(result)}</div>`;

    app.innerHTML = `
      <div class="panel">
        <div class="head" data-tauri-drag-region>
          <div class="titles" data-tauri-drag-region>
            <div class="title" data-tauri-drag-region>${esc(title)}</div>
            <div class="sub" data-tauri-drag-region>${esc(action?.name ?? "")}${lang ? ` · ${esc(lang)}` : ""}</div>
          </div>
          ${models.length > 1 ? `<select class="model" id="model" title="Model – přepnutí spustí akci znovu a uloží se jako výchozí">${models.map((m) => `<option value="${esc(m.id)}" ${m.id === usedModel ? "selected" : ""}>${esc(m.label)}</option>`).join("")}</select>` : `<span class="fh model-lbl">${esc(modelLabel(usedModel))}</span>`}
          ${running ? `<span class="status"><span class="spinner"></span> Pracuji…</span>` : ""}
          ${canDiff && !running && !error ? `<div class="seg"><button class="${view === "result" ? "on" : ""}" data-v="result">Výsledek</button><button class="${view === "diff" ? "on" : ""}" data-v="diff">Porovnání</button></div>` : ""}
          <button class="icon ${pinned ? "on" : ""}" id="pin" title="${pinned ? "Odepnout – panel se zavře při kliknutí jinam" : "Připnout – panel zůstane otevřený, i když klikneš jinam"}">${uiIcon("pin")}</button>
          <button class="icon" id="back" title="Zpět na akce (Backspace)">${uiIcon("back")}</button>
          <button class="icon" id="close" title="Zavřít (Esc)">${uiIcon("close")}</button>
        </div>
        <div class="body">${body}</div>
        <div class="foot">
          <button class="ghost" id="again" ${running ? "disabled" : ""}>↻ Znovu</button>
          <span class="spacer"></span>
          <button class="${isReplace ? "secondary" : "primary"}" id="copy" ${!result ? "disabled" : ""}>${copied ? "✓ Zkopírováno" : `Kopírovat <kbd>${isReplace ? "Ctrl+C" : "Enter"}</kbd>`}</button>
          <button class="${isReplace ? "primary" : "secondary"}" id="paste" ${running || error || !result ? "disabled" : ""}>Nahradit výběr <kbd>${isReplace ? "Enter" : "Ctrl+Enter"}</kbd></button>
        </div>
      </div>`;
    app.querySelector("#back")?.addEventListener("click", backToWheel);
    app.querySelector("#pin")?.addEventListener("click", () => { pinned = !pinned; invoke("set_popup_pinned", { pinned }); renderResult(); });
    app.querySelector("#close")?.addEventListener("click", () => invoke("close_popup"));
    app.querySelector("#again")?.addEventListener("click", () => { if (action) run(actions.indexOf(action), usedModel); });
    app.querySelector("#copy")?.addEventListener("click", doCopy);
    app.querySelector("#paste")?.addEventListener("click", doPaste);
    app.querySelectorAll<HTMLButtonElement>(".seg button").forEach((b) => b.addEventListener("click", () => { view = b.dataset.v as typeof view; renderResult(); }));
    app.querySelector<HTMLSelectElement>("#model")?.addEventListener("change", async (e) => {
      const spec = (e.target as HTMLSelectElement).value;
      try { await invoke("set_default_model", { spec }); defaultModel = spec; } catch { /* ignore */ }
      if (action) run(actions.indexOf(action), spec);
    });
    if (!running && !error) (document.getElementById(isReplace ? "paste" : "copy") as HTMLButtonElement | null)?.focus();
    fitPanel();
  }

  // Přizpůsobí okno obsahu: šířka 560, výška podle obsahu, max 60 % obrazovky.
  let lastH = 0;
  function fitPanel() {
    const panel = app.querySelector<HTMLElement>(".panel");
    if (!panel) return;
    const maxH = Math.floor(screen.availHeight * 0.6);
    const head = panel.querySelector<HTMLElement>(".head")!.offsetHeight;
    const foot = panel.querySelector<HTMLElement>(".foot")!.offsetHeight;
    const content = panel.querySelector<HTMLElement>(".result, .error");
    const bodyH = (content?.scrollHeight ?? 40) + 40; // + padding
    const h = Math.min(maxH, Math.max(160, head + foot + bodyH + 2));
    if (Math.abs(h - lastH) >= 6) {
      lastH = h;
      invoke("resize_popup", { width: Math.min(PANEL_W, screen.availWidth - 32), height: h });
    }
  }

  function backToWheel() {
    stage = "pick"; selected = -1; view = "result"; lastH = 0;
    invoke("resize_popup", { width: SIZE, height: SIZE });
    renderWheel();
  }
  const outText = () => (action?.mode === "replace" ? result : mdToPlain(result));
  function doCopy() {
    if (!result) return;
    invoke("copy_result", { text: outText() });
    copied = true; renderResult();
    setTimeout(() => { copied = false; invoke("close_popup"); }, 1500);
  }
  function doPaste() {
    if (running || error || !result) return;
    invoke("paste_result", { text: outText() });
  }

  async function run(i: number, modelOverride?: string) {
    action = actions[i];
    if (!action || !text) return;
    selected = i;
    stage = "result";
    result = ""; error = null; running = true; view = "result"; copied = false; lastH = 0;
    usedModel = modelOverride || action.model || defaultModel;
    renderResult();
    try {
      seq = await invoke<number>("run_action", { actionId: action.id, text, model: modelOverride ?? null });
    } catch (e) {
      error = String(e); running = false; renderResult();
    }
  }

  listen<PopupPayload>("popup-open", (ev) => {
    loadModels();
    pinned = false;
    invoke("ensure_ollama").catch(() => {});
    text = ev.payload.text;
    actions = ev.payload.actions;
    autoReplace = ev.payload.auto_replace;
    outputLang = ev.payload.output_language;
    selected = -1; centerHover = false; stage = "pick"; result = ""; error = null; running = false; copied = false; lastH = 0;
    if (ev.payload.auto_action) {
      const i = actions.findIndex((a) => a.id === ev.payload.auto_action);
      if (i >= 0 && text) { run(i); return; }
    }
    renderWheel();
  });

  let renderTimer: number | undefined;
  listen<TokenPayload>("llm-token", (ev) => {
    const p = ev.payload;
    if (p.seq !== seq) return;
    if (p.error) { error = p.error; running = false; renderResult(); return; }
    if (p.done) {
      running = false;
      result = result.trim();
      if (autoReplace && action?.mode === "replace" && result) { invoke("paste_result", { text: result }); return; }
      renderResult();
      return;
    }
    result += p.token;
    const el = document.getElementById("result");
    if (el) {
      if (el.classList.contains("md")) el.innerHTML = mdToHtml(result); else el.textContent = result;
      el.scrollTop = el.scrollHeight;
      // růst okna průběžně, ale ne při každém tokenu
      if (renderTimer === undefined) renderTimer = window.setTimeout(() => { renderTimer = undefined; fitPanel(); }, 120);
    } else renderResult();
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { invoke("close_popup"); return; }
    if (stage === "pick") {
      const n = actions.length;
      if (!n) return;
      if (e.key === "ArrowRight" || e.key === "ArrowDown") { selected = (selected + 1) % n; renderWheel(); }
      else if (e.key === "ArrowLeft" || e.key === "ArrowUp") { selected = (selected - 1 + n) % n; renderWheel(); }
      else if (e.key === "Enter" && selected >= 0) run(selected);
      else if (/^[1-9]$/.test(e.key) && +e.key <= n) run(+e.key - 1);
      else if (e.key.length === 1 && !e.ctrlKey && !e.altKey) {
        const i = actions.findIndex((a) => a.key && a.key.toLowerCase() === e.key.toLowerCase());
        if (i >= 0) run(i);
      }
    } else {
      if (e.key === "Enter") { e.preventDefault(); if (action?.mode === "replace" || e.ctrlKey) doPaste(); else doCopy(); }
      else if (e.key === "c" && e.ctrlKey) { e.preventDefault(); doCopy(); }
      else if (e.key === "Backspace" && !running) backToWheel();
      else if (e.key === "Tab" && action?.mode === "replace" && !running) { e.preventDefault(); view = view === "diff" ? "result" : "diff"; renderResult(); }
    }
  });

  renderWheel();
}

if (location.hash === "#settings") settings(app);
else popup();
