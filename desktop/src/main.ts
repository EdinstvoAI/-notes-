// Главный экран — порт `NotesWindow.swift`: шапка из планок, три капсулы, стеклянный лист,
// путь капсулой, дерево слева, раздел справа. Окно 1200×760, сжимается до 820.

import { api, type Api, type Folder, type Note } from "./api";
export type { Note };
import { icon, type Glyph } from "./icons";
import { drawSlats } from "./slats";
import { bindMarkdown, editorHTML } from "./editor";
import { closeSearch, openSearch, searchShown } from "./search";
import { showMenu, type MenuItem } from "./menu";
import { enableDrag, enableFileDrop, type DragHost } from "./drag";
import { mountCalendar } from "./calendar";
import { mountTasks } from "./tasks";
import { mountSettings } from "./settings";
import { addFiles, mountVideo } from "./video";
import type { Prefs } from "./api";

type Section = "notes" | "tasks" | "calendar" | "video" | "settings";
type Selection = { kind: "folder"; path: string } | { kind: "note"; path: string };

const SECTIONS: { id: Section; title: string; glyph: Glyph }[] = [
  { id: "notes", title: "заметки", glyph: "notes" },
  { id: "tasks", title: "задачи", glyph: "taskDone" },
  { id: "calendar", title: "календарь", glyph: "calendar" },
  { id: "video", title: "видео", glyph: "video" },
  { id: "settings", title: "настройки", glyph: "settings" },
];

const state = {
  section: "notes" as Section,
  tree: null as Folder | null,
  selection: { kind: "folder", path: "" } as Selection,
  expanded: new Set<string>(),
  text: "",
  saved: Date.now(),
  tabs: [] as string[],
  pins: [] as string[],
  /** Папка, в которой сейчас вписывают имя новой папки. */
  naming: null as string | null,
};

let backend: Api;
const app = document.getElementById("app")!;
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

// ——— тема: светлая/тёмная по системе, можно принудить ?theme=dark ———
const forced = new URLSearchParams(location.search).get("theme");
if (forced === "dark" || forced === "light") document.documentElement.dataset.theme = forced;
const isDark = () =>
  document.documentElement.dataset.theme === "dark" ||
  (!document.documentElement.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);

// ——— каркас окна: рисуется один раз, дальше меняются только части ———
app.innerHTML = `
  <canvas class="slats"></canvas>
  <header class="topbar arrive" style="transition-delay:.05s">
    <div class="group brand">
      <button class="logo" data-go="home">&lt;notes&gt;</button>
      <button class="search">${icon("search")}<span>поиск</span><kbd class="keycap">Ctrl K</kbd></button>
    </div>
    <nav class="group sections"></nav>
    <div class="group sync" title="история в git">${icon("git")}<span class="sync-text">без git</span></div>
  </header>
  <div class="sheet arrive" style="transition-delay:.15s"></div>
  <button class="crumb arrive" style="transition-delay:.2s"></button>
  <aside class="sidebar arrive" style="transition-delay:.2s"></aside>
  <main class="content arrive" style="transition-delay:.25s">
    <div class="actions"></div>
    <div class="page"></div>
  </main>`;

const $ = <T extends HTMLElement>(s: string) => app.querySelector<T>(s)!;
const slats = $<HTMLCanvasElement>(".slats");
const nav = $(".sections");
const crumb = $(".crumb");
const sidebar = $(".sidebar");
const actions = $(".actions");
let page = $(".page");

function paintSlats() {
  drawSlats(slats, app.clientWidth, 92, isDark());
}

// Шапка сжимается по очереди, как на маке: сначала текст синхронизации (значок остаётся), потом
// гибрид — значки и слово только у выбранного, потом одни значки. Ширину не считаем по формуле
// (на маке так и вышла кривая капсула), а меряем: влезло ли то, что нарисовано.
const topbar = $(".topbar");
const FITS = ["full", "nosync", "hybrid", "icons"] as const;
function fitHeader() {
  for (const fit of FITS) {
    topbar.dataset.fit = fit;
    if (topbar.scrollWidth <= topbar.clientWidth) return;
  }
}
new ResizeObserver(() => { paintSlats(); fitHeader(); }).observe(app);
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", paintSlats);

// ——— шапка ———
function renderNav() {
  nav.innerHTML = SECTIONS.map((s) => `
    <button class="section${s.id === state.section ? " active" : ""}" data-section="${s.id}" title="${s.title}">
      ${icon(s.glyph)}<span>${s.title}</span>
    </button>`).join("");
}

// ——— дерево ———
function findFolder(path: string, node = state.tree): Folder | null {
  if (!node) return null;
  if (node.path === path) return node;
  for (const f of node.folders) {
    const hit = findFolder(path, f);
    if (hit) return hit;
  }
  return null;
}

function rows(folder: Folder, depth: number): string {
  const pad = 8 + depth * 18;
  let html = state.naming === folder.path
    ? `<label class="row naming" style="padding-left:${pad}px">${icon("folder")}<input class="new-folder" placeholder="новая папка" spellcheck="false"></label>`
    : "";
  for (const f of folder.folders) {
    const selected = state.selection.kind === "folder" && state.selection.path === f.path;
    html += `<button class="row${selected ? " selected" : ""}" style="padding-left:${pad}px" data-folder="${esc(f.path)}">
      ${icon("folder")}<span>${esc(f.name)}</span></button>`;
    if (state.expanded.has(f.path)) html += rows(f, depth + 1);
  }
  for (const n of folder.notes) {
    const selected = state.selection.kind === "note" && state.selection.path === n.path;
    html += `<button class="row${selected ? " selected" : ""}" style="padding-left:${pad}px" data-note="${esc(n.path)}">
      ${icon("file")}<span>${esc(n.title)}</span></button>`;
  }
  return html;
}

function renderSidebar() {
  if (state.section === "notes") {
    // Закреплённые — над деревом, через линию (как на маке).
    const pins = state.pins.map((p) => {
      const selected = state.selection.kind === "note" && state.selection.path === p;
      return `<button class="row${selected ? " selected" : ""}" style="padding-left:8px" data-note="${esc(p)}" data-pinned>
        ${icon("file")}<span>${esc(p.split("/").pop()!.replace(/\.md$/, ""))}</span></button>`;
    }).join("");
    sidebar.innerHTML = (pins ? pins + `<hr class="pins-line">` : "") + (state.tree ? rows(state.tree, 0) : "");
    const input = sidebar.querySelector<HTMLInputElement>(".new-folder");
    if (input) {
      input.focus();
      const done = async (create: boolean) => {
        const parent = state.naming;
        state.naming = null;
        if (create && parent !== null && input.value.trim()) {
          const path = await backend.createFolder(parent, input.value);
          state.tree = await backend.tree();
          if (parent) state.expanded.add(parent);
          state.selection = { kind: "folder", path };
          render();
        } else renderSidebar();
      };
      input.addEventListener("keydown", (e) => { if (e.key === "Enter") void done(true); if (e.key === "Escape") void done(false); });
      input.addEventListener("blur", () => void done(true), { once: true });
    }
  } else if (state.section === "tasks") {
    // Фильтры рисует сам экран задач (tasks.ts).
  } else {
    sidebar.innerHTML = "";
  }
}

// ——— путь ———
function crumbText(): string {
  if (state.section === "notes") {
    const path = state.selection.path.replace(/\.md$/, "");
    return path ? path.split("/").join(" / ") : "все заметки";
  }
  return SECTIONS.find((s) => s.id === state.section)!.title;
}

// ——— раздел ———
const plural = (n: number, one: string, few: string, many: string) => {
  const d = n % 10, h = n % 100;
  return d === 1 && h !== 11 ? one : d >= 2 && d <= 4 && (h < 12 || h > 14) ? few : many;
};

function when(seconds: number): string {
  const date = new Date(seconds * 1000);
  const time = date.toLocaleTimeString("ru", { hour: "2-digit", minute: "2-digit" });
  const today = new Date();
  if (date.toDateString() === today.toDateString()) return `сегодня в ${time}`;
  return `${date.toLocaleDateString("ru", { day: "numeric", month: "short" })} в ${time}`;
}

const folderSvg = `<svg viewBox="0 0 184 133" aria-hidden="true"><path fill="currentColor" d="M0 20 A20 20 0 0 1 20 0 H42.9 A20 20 0 0 1 62.9 20 V10.5 A20 20 0 0 0 82.9 30.5 H164 A20 20 0 0 1 184 50.5 V113 A20 20 0 0 1 164 133 H20 A20 20 0 0 1 0 113 Z"/></svg>`;

function overviewHTML(folder: Folder): string {
  const root = folder.path === "";
  return `<div class="overview">
    <h1>${root ? "все заметки" : esc(folder.name)}</h1>
    <div class="counts">${folder.folders.length} ${plural(folder.folders.length, "папка", "папки", "папок")} | ${folder.note_count} ${plural(folder.note_count, "заметка", "заметки", "заметок")}</div>
    <div class="cards">${folder.folders.map((f) => `
      <button class="card" data-folder="${esc(f.path)}">${folderSvg}<b>${esc(f.name)}</b>
        <small>${f.note_count} ${plural(f.note_count, "заметка", "заметки", "заметок")}</small></button>`).join("")}
    </div>
    ${folder.notes.length ? `<div class="list-title">${root ? "без папки" : "заметки"}</div>
      ${folder.notes.map((n: Note) => `<button class="note-row" data-note="${esc(n.path)}">${icon("file")}
        <span>${esc(n.title)}</span><small>${when(n.modified)}</small></button>`).join("")}` : ""}
  </div>`;
}

function words(text: string) {
  return text.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).length;
}

function noteHTML(path: string): string {
  const title = path.split("/").pop()!.replace(/\.md$/, "");
  const n = words(state.text);
  return `<article class="note">
    <input class="note-title" value="${esc(title)}" placeholder="без названия" spellcheck="false">
    <div class="note-meta">изменено ${when(state.saved / 1000)} · ${n} ${plural(n, "слово", "слова", "слов")}</div>
    ${editorHTML(state.text)}
  </article>`;
}


function renderActions() {
  const buttons: [Glyph, string, string][] =
    state.section === "notes" ? [["microphone", "надиктовать", "dictate"], ["file", "новая заметка из шаблона", "templates"], ["newNote", "новая заметка", "new-note"]]
    : state.section === "tasks" ? [["microphone", "надиктовать", "dictate"], ["plus", "новая задача", "new-task"]]
    : state.section === "video" ? [["plus", "добавить видео или аудио", "add-video"]]
    : state.section === "calendar" ? [["microphone", "надиктовать", "dictate"]] : [];
  actions.innerHTML = buttons.map(([g, help, act]) =>
    `<button class="icon-button" title="${help}" data-act="${act}">${icon(g)}</button>`).join("");
}

function contentHTML(): string {
  if (!state.tree) {
    return `<div class="placeholder"><h1>папка с заметками</h1>
      <p>Выберите папку: заметки останутся обычными файлами Markdown, а мак откроет ту же папку.</p>
      <button class="primary" data-act="pick">выбрать папку</button></div>`;
  }
  switch (state.section) {
    case "notes":
      return state.selection.kind === "note" ? noteHTML(state.selection.path)
        : overviewHTML(findFolder(state.selection.path) ?? state.tree);
    case "tasks": return `<div class="tasks-root"></div>`;
    case "calendar": return `<div class="cal-root"></div>`;
    case "video": return `<div class="video-root"></div>`;
    case "settings": return `<div class="settings-root"></div>`;
  }
}

// Смена раздела или заметки: старое уходит в размытие, новое из него проявляется — как на маке.
let lastKey = "";
let tasksScreen: { focusNew(): void } | null = null;
function mountSection() {
  const cal = page.querySelector<HTMLElement>(".cal-root");
  if (cal) mountCalendar(cal, { backend, openNote });
  const tasks = page.querySelector<HTMLElement>(".tasks-root");
  const video = page.querySelector<HTMLElement>(".video-root");
  if (video) void mountVideo(video, sidebar, { openNote, reload, onCrumb: (t) => (crumb.textContent = t), inTauri: backend.inTauri });
  const settings = page.querySelector<HTMLElement>(".settings-root");
  if (settings) void mountSettings(settings, sidebar, { backend, onCrumb: (t) => (crumb.textContent = t), apply: applyPrefs,
    pickVault: async () => { const tree = await backend.pickVault(); if (tree) { state.tree = tree; state.tabs = []; state.selection = { kind: "folder", path: "" }; await reload(); } render(); },
    ensureFolder, refreshSync, sync: async () => { await syncWithGithub(); await refreshSync(); } });
  tasksScreen = tasks ? mountTasks(tasks, sidebar, { backend, openNote, onFilter: (t) => (crumb.textContent = t) }) : null;
}

function renderContent() {
  const key = `${state.section}:${state.selection.kind}:${state.selection.path}:${!!state.tree}`;
  const html = contentHTML();
  if (key === lastKey) { page.innerHTML = html; bindEditor(); mountSection(); return; }
  lastKey = key;
  const old = page;
  const next = document.createElement("div");
  next.className = "page entering";
  next.innerHTML = html;
  old.after(next);
  old.classList.add("leaving");
  requestAnimationFrame(() => requestAnimationFrame(() => next.classList.remove("entering")));
  setTimeout(() => old.remove(), 450);
  page = next;
  bindEditor();
  mountSection();
}

function render() {
  renderNav();
  fitHeader();
  renderTabs();
  renderSidebar();
  crumb.textContent = crumbText();
  renderActions();
  renderContent();
}

// ——— редактор: автосохранение через полсекунды тишины ———
let saveTimer = 0;
function bindEditor() {
  const editor = page.querySelector<HTMLElement>(".note-text");
  const title = page.querySelector<HTMLInputElement>(".note-title");
  if (!editor || !title || state.selection.kind !== "note") return;
  editor.spellcheck = prefs.spellcheck ?? true;
  bindMarkdown(editor, (text) => {
    state.text = text;
    const path = state.selection.path;
    clearTimeout(saveTimer);
    saveTimer = window.setTimeout(async () => {
      saveTimer = 0;
      await backend.write(path, state.text);
      state.saved = Date.now();
      const meta = page.querySelector(".note-meta");
      const n = words(state.text);
      if (meta) meta.textContent = `изменено ${when(state.saved / 1000)} · ${n} ${plural(n, "слово", "слова", "слов")}`;
    }, 500);
  });
  // Заголовок — имя файла: Enter или уход фокуса переименовывают файл.
  const rename = async () => {
    const from = state.selection.path;
    const next = title.value.trim();
    if (!next || next === from.split("/").pop()!.replace(/\.md$/, "")) { title.value = from.split("/").pop()!.replace(/\.md$/, ""); return; }
    try {
      const to = await backend.rename(from, next);
      state.tabs = state.tabs.map((p) => (p === from ? to : p));
      state.selection = { kind: "note", path: to };
      state.tree = await backend.tree();
      lastKey = `${state.section}:note:${to}:true`;
      title.value = to.split("/").pop()!.replace(/\.md$/, "");
      renderSidebar(); renderTabs(); crumb.textContent = crumbText();
    } catch {
      title.value = from.split("/").pop()!.replace(/\.md$/, "");
    }
  };
  title.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); editor.focus(); } });
  title.addEventListener("blur", rename);
  if (!state.text) title.select();
}

// ——— вкладки открытых заметок: капсулы в стиле пути, выбранная ярче, крестик при наведении ———
const tabsBar = document.createElement("div");
tabsBar.className = "tabs";
$(".content").prepend(tabsBar);

function renderTabs() {
  const show = state.section === "notes" && state.tabs.length > 0;
  tabsBar.hidden = !show;
  if (!show) return;
  tabsBar.innerHTML = state.tabs.map((p) => {
    const active = state.selection.kind === "note" && state.selection.path === p;
    return `<div class="tab${active ? " active" : ""}" data-tab="${esc(p)}"><span>${esc(p.split("/").pop()!.replace(/\.md$/, ""))}</span>
      <button class="tab-close" data-close="${esc(p)}" title="закрыть">×</button></div>`;
  }).join("");
}

function closeTab(path: string) {
  const i = state.tabs.indexOf(path);
  if (i < 0) return;
  state.tabs.splice(i, 1);
  if (state.selection.kind !== "note" || state.selection.path !== path) { renderTabs(); return; }
  if (state.tabs.length) void openNote(state.tabs[Math.min(i, state.tabs.length - 1)]);
  else { state.selection = { kind: "folder", path: path.split("/").slice(0, -1).join("/") }; render(); }
}

async function openNote(path: string) {
  state.text = await backend.read(path);
  state.saved = (findNote(path)?.modified ?? Date.now() / 1000) * 1000;
  state.selection = { kind: "note", path };
  state.section = "notes";
  if (!state.tabs.includes(path)) state.tabs.push(path);
  render();
}

function findNote(path: string, node = state.tree): Note | null {
  if (!node) return null;
  return node.notes.find((n) => n.path === path) ?? node.folders.map((f) => findNote(path, f)).find(Boolean) ?? null;
}

// ——— клики ———
app.addEventListener("click", async (e) => {
  const el = (e.target as HTMLElement).closest<HTMLElement>("button");
  const tab = (e.target as HTMLElement).closest<HTMLElement>(".tab");
  if (el?.dataset.close) { closeTab(el.dataset.close); return; }
  if (tab && !el) { await openNote(tab.dataset.tab!); return; }
  if (!el) return;
  if (el.classList.contains("search")) { openSearch(); return; }
  if (el.dataset.section) {
    state.section = el.dataset.section as Section;
    render();
  } else if (el.dataset.folder !== undefined) {
    const path = el.dataset.folder;
    if (el.classList.contains("row") && state.expanded.has(path) && state.selection.path === path) state.expanded.delete(path);
    else state.expanded.add(path);
    state.selection = { kind: "folder", path };
    render();
  } else if (el.dataset.note) {
    await openNote(el.dataset.note);
  } else if (el.dataset.go === "home" || el === crumb) {
    state.section = "notes";
    state.selection = { kind: "folder", path: "" };
    render();
  } else if (el.dataset.act === "pick") {
    state.tree = await backend.pickVault();
    render();
  } else if (el.dataset.act === "add-video") {
    page.querySelector<HTMLElement>("[data-pick]")?.click();
  } else if (el.dataset.act === "dictate") {
    if (backend.inTauri) await (await import("@tauri-apps/api/core")).invoke("dictation_toggle");
  } else if (el.dataset.act === "new-task") {
    tasksScreen?.focusNew();
  } else if (el.dataset.act === "templates" && state.tree) {
    const r = el.getBoundingClientRect();
    showMenu(r.right - 220, r.bottom + 8, await templateItems(currentFolder()));
  } else if (el.dataset.act === "new-note" && state.tree) {
    const folder = state.selection.kind === "folder" ? state.selection.path : state.selection.path.split("/").slice(0, -1).join("/");
    const path = await backend.create(folder, "Новая заметка");
    state.tree = await backend.tree();
    if (folder) state.expanded.add(folder);
    await openNote(path);
  }
});

async function ensureFolder(name: string) {
  if (!state.tree?.folders.some((f) => f.path === name)) await backend.createFolder("", name);
}

// ——— настройки интерфейса: тема, размер текста, орфография ———
let prefs: Prefs = {};
function applyPrefs(next: Prefs) {
  prefs = next;
  const root = document.documentElement;
  if (!forced) {
    if (next.theme === "light" || next.theme === "dark") root.dataset.theme = next.theme;
    else delete root.dataset.theme;
  }
  root.style.setProperty("--note-size", `${next.textSize ?? 15}px`);
  root.style.setProperty("--note-grow", `${(next.textSize ?? 15) - 15}px`);
  document.querySelectorAll<HTMLElement>(".note-text").forEach((e) => (e.spellcheck = next.spellcheck ?? true));
  paintSlats();
}

// ——— история в git: версия по расписанию, статус — в капсуле шапки ———
// На маке капсула говорит «синхронизировано», потому что версии уходят на GitHub. Здесь пока только
// локальная история, поэтому честно: «сохранено N мин назад».
const agoText = (seconds: number) => {
  const minutes = Math.floor((Date.now() / 1000 - seconds) / 60);
  return minutes < 1 ? "только что" : minutes < 60 ? `${minutes} мин назад` : `${Math.floor(minutes / 60)} ч назад`;
};

let syncError = false;
async function syncWithGithub() {
  try {
    const result = await backend.githubSync(now().date);
    syncError = false;
    applyPrefs({ ...prefs, lastSync: Date.now() });
    await backend.setPrefs(prefs);
    // Пришли правки с другого компьютера — перечитать дерево и открытую заметку.
    if (result.kind !== "upToDate") {
      await reload();
      if (state.selection.kind === "note") state.text = await backend.read(state.selection.path).catch(() => state.text);
      lastKey = "";
      render();
    }
  } catch {
    syncError = true;
  }
}

async function refreshSync() {
  const text = $(".sync-text");
  if (!state.tree) { text.textContent = "без git"; return; }
  try {
    const s = await backend.gitStatus();
    if (!s.repo || s.last_commit === null) { text.textContent = "без git"; fitHeader(); return; }
    const gh = await backend.githubStatus();
    const minutes = Math.floor((Date.now() / 1000 - s.last_commit) / 60);
    // Пора сохранить версию — раз в N минут из настроек (по умолчанию час, как на маке).
    if (minutes >= (prefs.commitMinutes ?? 60) && (await backend.gitCommit())) return refreshSync();
    if (gh.signed_in && gh.remote) {
      // С GitHub — как на маке: «синхронизировано N мин назад»; раз в час забираем и отправляем.
      if (!prefs.lastSync || Date.now() - prefs.lastSync > 60 * 60 * 1000) await syncWithGithub();
      text.textContent = syncError ? "не отправилось" : prefs.lastSync ? `синхронизировано ${agoText(prefs.lastSync / 1000)}` : "синхронизируем…";
    } else {
      // Только локальная история — честно «сохранено».
      text.textContent = `сохранено ${agoText(s.last_commit)}`;
    }
  } catch {
    text.textContent = "не сохранилось";
  }
  fitHeader();
}
setInterval(() => void refreshSync(), 30_000);

// ——— шаблоны: папка «шаблоны» хранилища, подстановки дата/время/название ———
const now = () => {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return { date: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`, time: `${pad(d.getHours())}:${pad(d.getMinutes())}` };
};
const currentFolder = () => (state.selection.kind === "folder" ? state.selection.path : state.selection.path.split("/").slice(0, -1).join("/"));

async function templateItems(folder: string): Promise<MenuItem[]> {
  const names = await backend.templates();
  const items: MenuItem[] = names.map((name) => ({ label: name, run: async () => {
    const { date, time } = now();
    const path = await backend.createFromTemplate(name, folder, date, time);
    await reload();
    if (folder) state.expanded.add(folder);
    await openNote(path);
  } }));
  if (!names.length) items.push({ label: "Шаблонов нет: добавьте .md в папку «шаблоны»", run: async () => {
    await ensureFolder("шаблоны");
    await reload(); state.expanded.add("шаблоны"); state.selection = { kind: "folder", path: "шаблоны" }; render();
  } });
  items.push("divider", { label: "Заметка дня", run: () => void openDaily() });
  return items;
}

async function openDaily() {
  const { date, time } = now();
  const path = await backend.daily(date, time);
  await reload();
  state.expanded.add("дневник");
  await openNote(path);
}

// ——— меню правого клика: как NoteMenu на маке ———
const revealLabel = () => (document.body.classList.contains("tauri-macos") ? "Показать в Finder" : "Показать в Проводнике");
const under = (path: string, root: string) => path === root || path.startsWith(root + "/");

async function reload() {
  state.tree = await backend.tree();
  state.pins = await backend.pins();
}

function itemsFor(path: string, folder: boolean): MenuItem[] {
  const items: MenuItem[] = [];
  if (!folder) {
    const pinned = state.pins.includes(path);
    items.push({ label: pinned ? "Открепить" : "Закрепить", run: async () => {
      state.pins = pinned ? state.pins.filter((p) => p !== path) : [path, ...state.pins];
      await backend.setPins(state.pins);
      renderSidebar();
    } });
  } else {
    items.push({ label: "Новая заметка", run: async () => {
      const note = await backend.create(path, "Новая заметка");
      await reload();
      if (path) state.expanded.add(path);
      await openNote(note);
    } });
    items.push({ label: "Новая из шаблона…", run: async () => {
      const r = sidebar.getBoundingClientRect();
      showMenu(r.left + 60, r.top + 40, await templateItems(path));
    } });
    items.push({ label: "Новая папка", run: () => {
      state.naming = path;
      if (path) state.expanded.add(path);
      state.section = "notes";
      renderSidebar();
    } });
  }
  if (path) {
    items.push({ label: "Дублировать", run: async () => { await backend.duplicate(path); await reload(); render(); } });
    if (backend.inTauri) items.push({ label: revealLabel(), run: () => void backend.reveal(path) });
    items.push("divider");
    items.push({ label: "Удалить", danger: true, run: async () => {
      await backend.trash(path);
      state.tabs = state.tabs.filter((p) => !under(p, path));
      state.pins = state.pins.filter((p) => !under(p, path));
      await backend.setPins(state.pins);
      if (under(state.selection.path, path)) state.selection = { kind: "folder", path: path.split("/").slice(0, -1).join("/") };
      await reload();
      render();
    } });
  }
  return items;
}

app.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement;
  if (target.closest(".note-text, .note-title, input")) return; // в тексте — меню правки ОС
  e.preventDefault();
  if (!state.tree) return;
  const el = target.closest<HTMLElement>("[data-note], [data-folder]");
  if (el?.dataset.note) showMenu(e.clientX, e.clientY, itemsFor(el.dataset.note, false));
  else if (el?.dataset.folder !== undefined) showMenu(e.clientX, e.clientY, itemsFor(el.dataset.folder, true));
  else if (target.closest(".sidebar, .overview")) showMenu(e.clientX, e.clientY, itemsFor(state.selection.kind === "folder" ? state.selection.path : "", true));
});

// ——— перетаскивание ———
const rebase = (p: string, from: string, to: string) => (under(p, from) ? to + p.slice(from.length) : p);
const dragHost: DragHost = {
  async move(path, folder) {
    const to = await backend.moveInto(path, folder);
    if (to === null) return;
    // Открытая заметка, вкладки и закреплённые едут вместе с файлом.
    state.tabs = state.tabs.map((p) => rebase(p, path, to));
    state.pins = state.pins.map((p) => rebase(p, path, to));
    await backend.setPins(state.pins);
    state.selection = { ...state.selection, path: rebase(state.selection.path, path, to) };
    state.expanded = new Set([...state.expanded].map((p) => rebase(p, path, to)));
    if (folder) state.expanded.add(folder);
    await reload();
    lastKey = `${state.section}:${state.selection.kind}:${state.selection.path}:true`;
    render();
  },
  async copyIn(paths, folder) {
    // На экране «видео» перетащенный файл — на расшифровку, а не в папку заметок.
    if (state.section === "video") { await addFiles(paths); lastKey = ""; render(); return; }
    for (const p of paths) await backend.copyIn(p, folder);
    if (folder) state.expanded.add(folder);
    await reload();
    render();
  },
};
enableDrag(sidebar, dragHost);

// Ctrl+K — поиск (на маке ⌘K), Ctrl+N — новая заметка.
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && searchShown()) { closeSearch(); return; }
  if (!(e.ctrlKey || e.metaKey)) return;
  if (e.key.toLowerCase() === "k") { e.preventDefault(); searchShown() ? closeSearch() : openSearch(); return; }
  if (e.key.toLowerCase() === "w" && state.selection.kind === "note") { e.preventDefault(); closeTab(state.selection.path); return; }
  if (e.key.toLowerCase() === "n") { e.preventDefault(); actions.querySelector<HTMLButtonElement>('[data-act="new-note"]')?.click(); }
});

// ——— старт ———
export const shell = { get backend() { return backend; }, get state() { return state; }, openNote, app };
(async () => {
  backend = await api;
  const os = await backend.os();
  document.body.classList.add(backend.inTauri ? `tauri-${os}` : "browser");
  applyPrefs(await backend.prefs());
  // Пинкод — до того, как показать заметки (как на маке: экран входа раньше главного).
  if (backend.inTauri) {
    const { lockApi, showUnlock } = await import("./lock");
    const lock = await lockApi.state();
    if (lock.needs_unlock) await showUnlock(lock);
    else if (lock.has_pin) await lockApi.seen();
    // Свернули надолго — при возврате спросить снова (15 минут, как на маке).
    let inactiveSince: number | null = null;
    addEventListener("blur", () => { inactiveSince = Math.floor(Date.now() / 1000); void lockApi.seen(); });
    addEventListener("focus", async () => {
      if (inactiveSince === null || document.querySelector(".unlock")) return;
      const since = inactiveSince;
      inactiveSince = null;
      if (await lockApi.neededOnReturn(since)) await showUnlock(await lockApi.state());
    });
  }
  state.tree = await backend.lastVault();
  if (state.tree) state.pins = await backend.pins();
  if (backend.inTauri) void enableFileDrop(dragHost);
  void refreshSync();
  // Правки снаружи (другой редактор, git, синхронизация): перечитать дерево; открытую заметку — если
  // своих несохранённых правок в ней нет, иначе своё не затираем (сохранится через полсекунды).
  if (backend.inTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    void listen<string[]>("vault-changed", async (e) => {
      await reload();
      const open = state.selection.kind === "note" ? state.selection.path : null;
      if (open && e.payload.includes(open) && !saveTimer) {
        const fresh = await backend.read(open).catch(() => null);
        if (fresh === null) {
          // Заметку удалили или перенесли снаружи — закрываем.
          state.tabs = state.tabs.filter((p) => p !== open);
          state.selection = { kind: "folder", path: open.split("/").slice(0, -1).join("/") };
          render();
          return;
        }
        // Своё же сохранение — экран не трогаем (курсор остаётся где был).
        if (fresh !== state.text) { state.text = fresh; lastKey = ""; render(); return; }
      }
      renderSidebar();
    });
  }
  const demo = new URLSearchParams(location.search).get("open");
  render();
  paintSlats();
  requestAnimationFrame(() => app.classList.add("arrived"));
  if (backend.inTauri && (await (await import("@tauri-apps/api/core")).invoke<boolean>("selftest"))) {
    const { runSelftest } = await import("./selftest");
    void runSelftest(backend, { openNote, app });
  }
  // Заметка на каждый день — открывается при запуске, если включена в настройках → шаблоны.
  if (state.tree && prefs.daily && !demo) await openDaily();
  if (demo) {
    for (const part of demo.split("/").slice(0, -1).reduce<string[]>((acc, p) => [...acc, acc.length ? `${acc.at(-1)}/${p}` : p], [])) state.expanded.add(part);
    await openNote(demo);
  }
})();
