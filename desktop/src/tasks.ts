// Экран задач — порт `TasksScreen.swift`: «N открыто | M с датой», строка новой задачи, группы
// «сегодня / на неделе / позже / без даты / готово», фильтры в боковой панели. Отметили — галочка и
// зачёркивание сразу, строка уходит через полсекунды; правится только квадратик в строке заметки.

import type { Api, TaskRow } from "./api";
import { icon, type Glyph } from "./icons";

type Filter = "all" | "today" | "week" | "noDate" | "dated" | "done";
const FILTERS: [Filter, string, Glyph][] = [
  ["all", "все задачи", "task"], ["today", "сегодня", "task"], ["week", "на неделе", "task"],
  ["noDate", "без даты", "task"], ["dated", "с датой", "calendar"], ["done", "готово", "taskDone"],
];
type Group = "сегодня" | "на неделе" | "позже" | "без даты" | "готово";
const GROUPS: Group[] = ["сегодня", "на неделе", "позже", "без даты", "готово"];
const INBOX = "Входящие.md";

export interface TasksHost {
  backend: Api;
  /** Задачи без исключённых папок (настройки → календарь). */
  tasks(): Promise<TaskRow[]>;
  openNote(path: string): Promise<void>;
  /** Сколько открытых — в шапку боковой панели и путь. */
  onFilter(title: string): void;
  /** Положить шаг в Ctrl+Z приложения. */
  undoable(label: string, run: () => Promise<void>): void;
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const addDays = (n: number) => { const d = new Date(); d.setDate(d.getDate() + n); return iso(d); };

let filter: Filter = "all";

// Просроченное — тоже «сегодня»: его надо сделать сейчас, а не потерять в прошлом (как на маке).
export function groupOf(t: TaskRow, today = iso(new Date()), weekEnd = addDays(7)): Group {
  if (t.done) return "готово";
  if (!t.date) return "без даты";
  if (t.date <= today) return "сегодня";
  return t.date < weekEnd ? "на неделе" : "позже";
}

function visible(all: TaskRow[]): TaskRow[] {
  switch (filter) {
    case "all": return all.filter((t) => !t.done);
    case "today": return all.filter((t) => groupOf(t) === "сегодня");
    case "week": return all.filter((t) => ["сегодня", "на неделе"].includes(groupOf(t)));
    case "noDate": return all.filter((t) => !t.done && !t.date);
    case "dated": return all.filter((t) => !t.done && t.date);
    case "done": return all.filter((t) => t.done);
  }
}

function when(t: TaskRow): string {
  if (!t.date) return "";
  const today = iso(new Date());
  const day = t.date === today ? "сегодня"
    : new Date(t.date + "T12:00").toLocaleDateString("ru", { weekday: "short", day: "numeric", month: "short" }).replaceAll(".", "").replaceAll(",", "");
  return t.time ? `${day}, ${t.time}` : day;
}

export function mountTasks(root: HTMLElement, sidebar: HTMLElement, host: TasksHost) {
  let all: TaskRow[] = [];
  let withDate = false;
  let urgent = false;

  function renderSidebar() {
    const count = (f: Filter) => { const saved = filter; filter = f; const n = visible(all).length; filter = saved; return n; };
    sidebar.innerHTML = FILTERS.map(([f, title, glyph]) => `
      <button class="row${f === filter ? " selected" : ""}" style="padding-left:8px" data-filter="${f}">
        ${icon(glyph)}<span>${title}</span><small class="count">${count(f)}</small></button>`).join("");
    host.onFilter(`задачи / ${FILTERS.find(([f]) => f === filter)![1]}`);
  }

  function row(t: TaskRow): string {
    const i = all.indexOf(t);
    const path = t.note.replace(/\.md$/, "").split("/").join(" / ");
    return `<div class="task-row${t.done ? " done" : ""}" data-i="${i}">
      <button class="check" data-check="${i}" title="${t.done ? "вернуть" : "готово"}">${icon(t.done ? "taskDone" : "task")}</button>
      ${t.urgent ? `<b class="urgent" title="срочно">!!</b>` : ""}
      <button class="task-title" data-open="${i}">${esc(t.title)}</button>
      <small class="task-note">${esc(path)}</small>
      <small class="task-when${t.date ? " dated" : ""}">${when(t)}</small>
    </div>`;
  }

  function renderList() {
    const list = visible(all);
    const open = all.filter((t) => !t.done).length;
    const dated = all.filter((t) => !t.done && t.date).length;
    root.querySelector(".tasks-counts")!.textContent = `${open} открыто | ${dated} с датой`;
    const body = root.querySelector(".tasks-groups")!;
    if (!list.length) {
      body.innerHTML = `<p class="tasks-empty">${all.length ? "здесь пусто" : "задач нет. добавьте выше или строкой «- [ ] дело» в заметке"}</p>`;
      return;
    }
    body.innerHTML = GROUPS.map((g) => {
      const items = list.filter((t) => groupOf(t) === g)
        .sort((a, b) => `${a.date ?? "9"}${a.time ?? ""}`.localeCompare(`${b.date ?? "9"}${b.time ?? ""}`));
      return items.length ? `<h2>${g}</h2>${items.map(row).join("")}` : "";
    }).join("");
  }

  async function reload() {
    all = await host.tasks();
    renderSidebar();
    renderList();
  }

  const now = new Date();
  const local = `${iso(now)}T18:00`;
  root.innerHTML = `<div class="tasks">
    <h1>задачи</h1><p class="tasks-counts"></p>
    <label class="new-task">${icon("plus")}
      <input class="new-title" placeholder="новая задача" spellcheck="false">
      <input class="new-date" type="datetime-local" value="${local}" hidden>
      <button class="new-when" title="со сроком попадёт в календарь">${icon("calendar")}</button>
      <button class="new-urgent" title="срочно">!!</button>
    </label>
    <div class="tasks-groups"></div></div>`;

  const title = root.querySelector<HTMLInputElement>(".new-title")!;
  const date = root.querySelector<HTMLInputElement>(".new-date")!;
  const whenButton = root.querySelector<HTMLElement>(".new-when")!;
  const urgentButton = root.querySelector<HTMLElement>(".new-urgent")!;

  whenButton.addEventListener("click", (e) => {
    e.preventDefault();
    withDate = !withDate;
    date.hidden = !withDate;
    whenButton.classList.toggle("on", withDate);
  });
  urgentButton.addEventListener("click", (e) => { e.preventDefault(); urgent = !urgent; urgentButton.classList.toggle("on", urgent); });
  title.addEventListener("keydown", async (e) => {
    if (e.key !== "Enter" || !title.value.trim()) return;
    const [d, t] = withDate && date.value ? date.value.split("T") : [null, null];
    await host.backend.appendTask(INBOX, title.value + (urgent ? " !!" : ""), d, t ?? null, null);
    title.value = "";
    urgent = false;
    urgentButton.classList.remove("on");
    await reload();
  });

  root.addEventListener("click", async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    if (b.dataset.check) {
      const t = all[Number(b.dataset.check)];
      const rowEl = b.closest<HTMLElement>(".task-row")!;
      rowEl.classList.toggle("checking");
      b.innerHTML = icon(t.done ? "task" : "taskDone");
      try {
        await host.backend.setDone(t, !t.done);
        // Ctrl+Z — снять отметку: строка теперь с другим квадратиком, её и ищем.
        const raw = t.raw.replace(/\[[ xX]\]/, t.done ? "[ ]" : "[x]");
        host.undoable("отметка задачи", () => host.backend.setDone({ ...t, raw, done: !t.done }, t.done));
      } catch { /* поменяли снаружи — перечитаем */ }
      setTimeout(() => { rowEl.classList.add("leaving"); setTimeout(() => void reload(), 450); }, 500);
    } else if (b.dataset.open) {
      await host.openNote(all[Number(b.dataset.open)].note);
    }
  });
  sidebar.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-filter]");
    if (!b || !root.isConnected) return;
    filter = b.dataset.filter as Filter;
    renderSidebar();
    renderList();
  });

  void reload();
  return { focusNew: () => title.focus() };
}
