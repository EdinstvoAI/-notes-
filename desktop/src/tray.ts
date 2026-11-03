// Панель значка в трее — порт `MenuBarPanel.swift` (03.11.2026): сейф под рукой — найти, скопировать
// пароль или код 2FA, плюс «открыть <notes>» и «надиктовать». Сейф тот же, что в окне (общий в Rust).

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { languageFor, setLanguage } from "./i18n";

interface Item { id: string; kind: "login" | "totp" | "secret"; title: string; username: string; url: string }
interface State { exists: boolean; remembered: boolean; unlocked: boolean; hello: boolean; vault: boolean }

const root = document.querySelector<HTMLElement>(".tray")!;
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
let query = "", copied = "", items: Item[] = [], codes: Record<string, string> = {};

async function paint() {
  const s = await invoke<State>("safe_state").catch(() => null);
  let body = "";
  if (s) {
    if (s.unlocked) {
      items = await invoke<Item[]>("safe_items");
      [codes] = await invoke<[Record<string, string>, number]>("safe_codes");
      const words = query.toLowerCase();
      const found = items.filter((i) => !words || `${i.title} ${i.username} ${i.url}`.toLowerCase().includes(words));
      body = `<input class="safe-input" data-q placeholder="найти пароль" value="${esc(query)}" spellcheck="false">
        <div class="tray-list">${found.map((i) => `<button class="tray-row" data-copy="${i.id}"><div><span>${esc(i.title)}</span><small>${esc(i.username)}</small></div>
          ${i.kind === "totp" ? `<b>${codes[i.id] ?? "—"}</b>` : ""}<small>${copied === i.id ? "скопировано" : "скопировать"}</small></button>`).join("")}</div>
        <button class="quiet" data-act="lock">закрыть сейф</button>`;
    } else if (s.remembered && s.hello) body = `<button class="pill" data-act="hello">открыть по windows hello</button>`;
    else if (s.exists) body = `<form data-form><input class="safe-input" type="password" name="p" placeholder="мастер-пароль"></form>`;
    else body = `<small>сейфа ещё нет — создай его в разделе «пароли»</small>`;
  }
  root.innerHTML = `${body}<hr><div class="tray-foot"><button class="quiet" data-act="open">открыть &lt;notes&gt;</button>
    <button class="quiet" data-act="dictate">надиктовать</button></div>`;
  const q = root.querySelector<HTMLInputElement>("[data-q]");
  if (q) { q.focus(); q.setSelectionRange(query.length, query.length); }
}

root.onclick = async (e) => {
  const b = (e.target as HTMLElement).closest<HTMLElement>("[data-copy],[data-act]");
  if (!b) return;
  if (b.dataset.copy) { await invoke("safe_copy", { what: "item", id: b.dataset.copy }); copied = b.dataset.copy; }
  else if (b.dataset.act === "lock") await invoke("safe_lock");
  else if (b.dataset.act === "hello") await invoke("safe_unlock_hello");
  else if (b.dataset.act === "open") { await invoke("show_main"); await getCurrentWindow().hide(); return; }
  else if (b.dataset.act === "dictate") { await getCurrentWindow().hide(); await invoke("dictation_toggle"); return; }
  void invoke<string>("prefs").then((p) => setLanguage(languageFor(JSON.parse(p).uiLanguage))).catch(() => {});
void paint();
};
root.oninput = (e) => { const t = e.target as HTMLInputElement; if (t.dataset.q !== undefined) { query = t.value; void paint(); } };
root.onsubmit = async (e) => {
  e.preventDefault();
  const p = (root.querySelector("[name=p]") as HTMLInputElement).value;
  await invoke("safe_unlock", { text: p }).catch(() => {});
  void invoke<string>("prefs").then((p) => setLanguage(languageFor(JSON.parse(p).uiLanguage))).catch(() => {});
void paint();
};
// Панель показали — свежее состояние; ушли — прячемся, как поповер на маке.
getCurrentWindow().onFocusChanged(({ payload }) => { if (payload) { copied = ""; void paint(); } else void getCurrentWindow().hide(); });
setInterval(() => { if (items.some((i) => i.kind === "totp") && document.hasFocus()) void paint(); }, 5000);
void invoke<string>("prefs").then((p) => setLanguage(languageFor(JSON.parse(p).uiLanguage))).catch(() => {});
void paint();
