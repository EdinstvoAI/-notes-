// Поиск поверх экрана — порт `SearchOverlay` (кадр 79:12603): капсула 720 по центру, сверху 150,
// поле крупным шрифтом, «недавние» без запроса, результаты группами «заметки» и «задачи»,
// ↑↓ — выбрать, Enter — открыть, Esc — закрыть. Экран под ним размыт.

import type { Hit, Note, TaskRow } from "./api";
import { icon, type Glyph } from "./icons";
import { shell } from "./main";

type Result = { kind: "заметки" | "задачи" | "по смыслу"; title: string; detail: string; snippet: string | null; path: string; glyph: Glyph };

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const folderOf = (path: string) => {
  const parts = path.split("/").slice(0, -1);
  return parts.length ? parts.join(" / ") : "без папки";
};

let overlay: HTMLElement | null = null;
let results: Result[] = [];
let highlighted = 0;
let token = 0;

export const searchShown = () => overlay !== null;

function allNotes(): Note[] {
  const out: Note[] = [];
  const walk = (f: NonNullable<typeof shell.state.tree>) => { out.push(...f.notes); f.folders.forEach(walk); };
  if (shell.state.tree) walk(shell.state.tree);
  return out;
}

function highlight(text: string, word: string): string {
  const i = word ? text.toLowerCase().replaceAll("ё", "е").indexOf(word.toLowerCase().replaceAll("ё", "е")) : -1;
  if (i < 0) return esc(text);
  return `${esc(text.slice(0, i))}<b>${esc(text.slice(i, i + word.length))}</b>${esc(text.slice(i + word.length))}`;
}

function renderResults(query: string) {
  const list = overlay!.querySelector(".search-results")!;
  const count = overlay!.querySelector(".search-count")!;
  const word = query.trim().split(/\s+/)[0] ?? "";
  count.textContent = query.trim() ? String(results.length) : "";
  if (!query.trim()) {
    const recent = allNotes().sort((a, b) => b.modified - a.modified).slice(0, 5);
    results = recent.map((n) => ({ kind: "заметки", title: n.title, detail: folderOf(n.path), snippet: null, path: n.path, glyph: "file" }));
    list.innerHTML = `<div class="caption">недавние</div>` + results.map((r, i) => row(r, i, "")).join("");
    return;
  }
  if (!results.length) { list.innerHTML = `<div class="nothing">ничего не нашлось</div>`; return; }
  list.innerHTML = results.map((r, i) =>
    (i === 0 || results[i - 1].kind !== r.kind ? `<div class="caption">${r.kind}</div>` : "") + row(r, i, word)).join("");
}

function row(r: Result, i: number, word: string): string {
  return `<button class="result${i === highlighted ? " selected" : ""}" data-index="${i}">
    ${icon(r.glyph)}
    <div><div class="result-head"><span>${highlight(r.title, word)}</span><small>${esc(r.detail)}</small></div>
    ${r.snippet ? `<div class="snippet">${highlight(r.snippet, word)}</div>` : ""}</div>
  </button>`;
}

async function query(text: string) {
  const mine = ++token;
  highlighted = 0;
  if (!text.trim()) { renderResults(text); return; }
  const [hits, tasks] = await Promise.all([shell.backend.search(text), shell.backend.tasks()]);
  if (mine !== token || !overlay) return;
  const norm = (s: string) => s.toLowerCase().replaceAll("ё", "е");
  const words = norm(text).split(/\s+/).filter(Boolean);
  results = [
    ...hits.map((h: Hit): Result => ({ kind: "заметки", title: h.note.title, detail: folderOf(h.note.path), snippet: h.snippet, path: h.note.path, glyph: "file" })),
    ...tasks.filter((t: TaskRow) => words.every((w) => norm(t.title).includes(w))).slice(0, 8)
      .map((t: TaskRow): Result => ({ kind: "задачи", title: t.title, detail: [t.date, t.time].filter(Boolean).join(" ") || folderOf(t.note),
        snippet: null, path: t.note, glyph: "taskDone" })),
  ];
  renderResults(text);
  // По смыслу — модель на этом компьютере (если скачана): заметки, где нет тех же слов.
  if (!shell.backend.inTauri) return;
  const { invoke } = await import("@tauri-apps/api/core");
  const meaning = await invoke<[string, number][]>("meaning_search", { query: text }).catch(() => []);
  if (mine !== token || !overlay || !meaning.length) return;
  const shown = new Set(results.map((r) => r.path));
  const best = meaning[0][1];
  const extra = meaning.filter(([p, s]) => !shown.has(p) && s >= 0.78 && s >= best - 0.05).slice(0, 5);
  results.push(...extra.map(([path]): Result => ({ kind: "по смыслу", title: path.split("/").pop()!.replace(/\.md$/, ""),
    detail: folderOf(path), snippet: null, path, glyph: "file" })));
  renderResults(text);
}

function choose(i: number) {
  const r = results[i];
  if (!r) return;
  closeSearch();
  void shell.openNote(r.path);
}

export function openSearch() {
  if (overlay) return;
  shell.app.classList.add("searching");
  overlay = document.createElement("div");
  overlay.className = "search-overlay";
  overlay.innerHTML = `
    <div class="search-panel" role="dialog" aria-label="поиск">
      <label class="search-field">${icon("search", 20)}<input placeholder="найти" spellcheck="false"><span class="search-count"></span></label>
      <div class="search-results"></div>
      <footer><span><kbd class="keycap">↑↓</kbd> выбрать</span><span><kbd class="keycap">enter</kbd> открыть</span>
        <span><kbd class="keycap">esc</kbd> закрыть</span><em>заметки, задачи и по смыслу</em></footer>
    </div>`;
  document.body.append(overlay);
  const input = overlay.querySelector("input")!;
  input.addEventListener("input", () => void query(input.value));
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!results.length) return;
      highlighted = (highlighted + (e.key === "ArrowDown" ? 1 : -1) + results.length) % results.length;
      overlay!.querySelectorAll(".result").forEach((el, i) => el.classList.toggle("selected", i === highlighted));
      overlay!.querySelector(".result.selected")?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter") { e.preventDefault(); choose(highlighted); }
  });
  overlay.addEventListener("mousedown", (e) => { if (e.target === overlay) closeSearch(); });
  overlay.addEventListener("click", (e) => {
    const r = (e.target as HTMLElement).closest<HTMLElement>(".result");
    if (r) choose(Number(r.dataset.index));
  });
  renderResults("");
  requestAnimationFrame(() => { overlay?.classList.add("shown"); input.focus(); });
}

export function closeSearch() {
  if (!overlay) return;
  const gone = overlay;
  overlay = null;
  shell.app.classList.remove("searching");
  gone.classList.remove("shown");
  setTimeout(() => gone.remove(), 300);
}
