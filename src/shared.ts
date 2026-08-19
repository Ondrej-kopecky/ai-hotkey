// Sdílené typy a helpery pro popup i nastavení.
// ---- typy (zrcadlí Rust struktury v config.rs / actions.rs) ----
export interface Action { id: string; name: string; icon: string; prompt: string; mode: "replace" | "show"; key: string; hotkey: string; enabled: boolean; description: string; model: string }
export interface ModelInfo { id: string; label: string; provider: string }
export interface Config {
  hotkey: string;
  provider: "ollama" | "anthropic";
  ollama: { url: string; model: string };
  anthropic: { api_key: string; model: string };
  actions: Action[];
  target_language: string;
  output_language: string;
  auto_replace: boolean;
  leo_bridge: { enabled: boolean; port: number };
}
export interface PopupPayload { text: string; actions: Action[]; auto_action: string | null; auto_replace: boolean; output_language: string }
export interface TokenPayload { seq: number; token: string; done: boolean; error: string | null }

export const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));

// Fluent ikony (20×20, stroke) podle Action.icon
export const ICONS: Record<string, string> = {
  translate: '<circle cx="10" cy="10" r="7.5"/><path d="M2.5 10h15M10 2.5c-2.5 2.5-2.5 12.5 0 15M10 2.5c2.5 2.5 2.5 12.5 0 15"/>',
  list: '<path d="M4 5.5h12M4 10h12M4 14.5h8"/>',
  pen: '<path d="M4 16l1-4 8.5-8.5a1.5 1.5 0 0 1 2.1 0l.9.9a1.5 1.5 0 0 1 0 2.1L8 15l-4 1z"/><path d="M12 5l3 3"/>',
  check: '<path d="M4 5.5h9M4 9.5h6"/><path d="M11 13.5l2 2 4-4.5"/>',
  bulb: '<path d="M7.5 14.5h5M8.5 17h3M10 3a5 5 0 0 0-3 9c.6.5 1 1.2 1 2h4c0-.8.4-1.5 1-2a5 5 0 0 0-3-9z"/>',
  pin: '<path d="M12.5 3.5l4 4-1.2 1.2-.6-.2-3.2 3.2.3 2.6-1.4 1.4L7.5 12.8 4 16.3l-.4-.4 3.5-3.5-2.9-2.9 1.4-1.4 2.6.3 3.2-3.2-.2-.6z"/>',
  back: '<path d="M12.5 4.5L7 10l5.5 5.5"/>',
  close: '<path d="M5.5 5.5l9 9M14.5 5.5l-9 9"/>',
  spark: '<path d="M10 3l1.6 4.4L16 9l-4.4 1.6L10 15l-1.6-4.4L4 9l4.4-1.6z"/>',
};
export const iconSvg = (name: string, x: number, y: number) =>
  `<g transform="translate(${x - 11},${y - 11}) scale(1.1)" class="ico">${ICONS[name] ?? ICONS.spark}</g>`;

