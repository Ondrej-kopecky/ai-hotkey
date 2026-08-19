// Mini Markdown pro výsledkový panel – bezpečné (nejdřív escape, pak formát).
// Podporuje: odrážky (- * •), číslované seznamy, nadpisy, **tučné**, *kurzívu*, `kód`, ```bloky```.
import { esc } from "./shared";

function inline(t: string): string {
  return esc(t)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[^*\w])\*(?!\s)([^*]+?)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/(^|[^_\w])_(?!\s)([^_]+?)_(?!\w)/g, "$1<em>$2</em>");
}

export function mdToHtml(src: string): string {
  const lines = src.replace(/\r/g, "").split("\n");
  const out: string[] = [];
  let list: "ul" | "ol" | null = null;
  let inCode = false;
  const closeList = () => { if (list) { out.push(`</${list}>`); list = null; } };
  for (const raw of lines) {
    if (raw.trim().startsWith("```")) { closeList(); inCode = !inCode; out.push(inCode ? "<pre><code>" : "</code></pre>"); continue; }
    if (inCode) { out.push(esc(raw) + "\n"); continue; }
    const l = raw.trimEnd();
    let m: RegExpMatchArray | null;
    if ((m = l.match(/^\s*([-*•])\s+(.*)$/))) { if (list !== "ul") { closeList(); list = "ul"; out.push("<ul>"); } out.push(`<li>${inline(m[2])}</li>`); continue; }
    if ((m = l.match(/^\s*(\d+)[.)]\s+(.*)$/))) { if (list !== "ol") { closeList(); list = "ol"; out.push("<ol>"); } out.push(`<li>${inline(m[2])}</li>`); continue; }
    closeList();
    if ((m = l.match(/^(#{1,6})\s+(.*)$/))) { out.push(`<h4>${inline(m[2])}</h4>`); continue; }
    if (l.trim() === "") { out.push('<div class="gap"></div>'); continue; }
    out.push(`<p>${inline(l)}</p>`);
  }
  closeList();
  return out.join("");
}

// Markdown → čistý text pro schránku (bez hvězdiček, odrážky jako •)
export function mdToPlain(src: string): string {
  return src.replace(/\r/g, "")
    .replace(/^\s*[-*]\s+/gm, "• ")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/\*\*(.+?)\*\*/g, "$1")
    .replace(/(^|[^*\w])\*(?!\s)([^*]+?)\*(?!\w)/g, "$1$2")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
