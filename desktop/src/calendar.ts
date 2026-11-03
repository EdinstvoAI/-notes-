// Календарь — порт `CalendarScreen` (TasksScreen.swift): день, неделя, месяц; задачи с `📅` из заметок.
// На маке события живут в Apple Calendar через EventKit. На Windows системный календарь закрыт для
// обычных программ (только приложения из Store), поэтому календарь — внутри приложения, по файлам.
//
// Клик по пустому месту — новая задача на это время (дописывается во «Входящие.md», как на маке),
// событие тащится мышью на другой день и время (шаг 15 минут), клик по событию — открыть заметку.

import type { Api, TaskRow } from "./api";
import { icon } from "./icons";

type Mode = "day" | "week" | "month";
const MODES: [Mode, string][] = [["day", "день"], ["week", "неделя"], ["month", "месяц"]];
const HOUR = 44;
const INBOX = "Входящие.md";

export interface CalendarHost {
  backend: Api;
  /** Задачи без исключённых папок (настройки → календарь). */
  tasks(): Promise<TaskRow[]>;
  openNote(path: string): Promise<void>;
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const toMin = (t: string) => Number(t.slice(0, 2)) * 60 + Number(t.slice(3, 5));
const hhmm = (m: number) => `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;
const addDays = (d: Date, n: number) => { const x = new Date(d); x.setDate(x.getDate() + n); return x; };
const startOfWeek = (d: Date) => { const x = new Date(d.getFullYear(), d.getMonth(), d.getDate()); return addDays(x, -((x.getDay() + 6) % 7)); };

const view = { mode: "week" as Mode, anchor: new Date() };

function days(): Date[] {
  if (view.mode === "day") return [new Date(view.anchor.getFullYear(), view.anchor.getMonth(), view.anchor.getDate())];
  const start = startOfWeek(view.anchor);
  return Array.from({ length: 7 }, (_, i) => addDays(start, i));
}

function title(): string {
  const month = (d: Date) => d.toLocaleDateString("ru", { day: "numeric", month: "long" });
  if (view.mode === "day") return month(view.anchor);
  if (view.mode === "week") { const d = days(); return `${d[0].getDate()} – ${month(d[6])}`; }
  return view.anchor.toLocaleDateString("ru", { month: "long", year: "numeric" }).replace(" г.", "");
}

function shift(step: number) {
  const a = new Date(view.anchor);
  if (view.mode === "day") a.setDate(a.getDate() + step);
  else if (view.mode === "week") a.setDate(a.getDate() + step * 7);
  else a.setMonth(a.getMonth() + step);
  view.anchor = a;
}

/** Место среди пересекающихся, как в Календаре Apple (порт `TasksScreen.lanes`, 01.11.2026): начавшиеся
 *  почти вместе (до 45 мин) стоят рядом и делят ширину, начавшееся позже ложится слоем со сдвигом вправо
 *  и не закрывает чужое название. level — глубина слоя, col и count — место в ряду «рядом». */
export interface Lane { level: number; col: number; count: number }
export function lanes(list: TaskRow[]): Map<TaskRow, Lane> {
  const out = new Map<TaskRow, Lane>();
  const start = (t: TaskRow) => toMin(t.time!), end = (t: TaskRow) => start(t) + (t.minutes ?? 60);
  const placed: TaskRow[] = [];
  const rows = new Map<TaskRow, TaskRow[]>();
  for (const t of [...list].sort((a, b) => start(a) - start(b))) {
    const active = placed.filter((p) => end(p) > start(t));
    const near = active.find((p) => start(t) - start(p) < 45);
    const head = near && [...rows].find(([, r]) => r.includes(near))?.[0];
    if (near && head) {
      const row = rows.get(head)!;
      row.push(t);
      out.set(t, { level: out.get(near)!.level, col: row.length - 1, count: 1 });
      for (const r of row) out.get(r)!.count = row.length;
    } else {
      out.set(t, { level: Math.max(-1, ...active.map((p) => out.get(p)!.level)) + 1, col: 0, count: 1 });
      rows.set(t, [t]);
    }
    placed.push(t);
  }
  return out;
}

/** Отступ и ширина в колонке шириной colW: глубже двух слоёв не сдвигаем; узко — следующий на правую половину. */
export function laneBox(l: Lane, colW: number): { left: number; width: number } {
  const base = Math.min(l.level, 2) * Math.min(18, (colW - 6) * 0.2);
  const region = colW - 6 - base;
  const fits = region / l.count >= 70;
  const left = base + (fits ? (l.col * region) / l.count : (l.col * region) / 2 / Math.max(1, l.count - 1));
  return { left: 3 + left, width: fits ? region / l.count : region - (left - base) };
}

export function mountCalendar(root: HTMLElement, host: CalendarHost) {
  let tasks: TaskRow[] = [];

  const controls = () => `
    <div class="cal-controls">
      <button class="icon-button" data-cal="prev" title="назад">${icon("chevronLeft")}</button>
      <button class="cal-today" data-cal="today">сегодня</button>
      <button class="icon-button" data-cal="next" title="вперёд">${icon("chevronRight")}</button>
      <button class="icon-button" data-cal="new" title="новая задача">${icon("plus")}</button>
      <button class="cal-today" data-cal="ics" title="задачи в Календарь Windows или Outlook: откройте файл .ics">.ics</button>
      <div class="cal-modes">${MODES.map(([m, t]) => `<button class="cal-mode${m === view.mode ? " on" : ""}" data-mode="${m}">${t}</button>`).join("")}</div>
    </div>`;

  function gridHTML(): string {
    const list = days();
    const today = iso(new Date());
    const cols = list.map((d) => {
      const key = iso(d);
      const timed = tasks.filter((t) => t.date === key && t.time);
      const placed = lanes(timed);
      const colW = Math.max(60, (root.clientWidth - 24 - 15 - 52) / list.length);
      const events = timed.map((t) => {
        const lane = placed.get(t)!;
        const box = laneBox(lane, colW);
        const top = (toMin(t.time!) / 60) * HOUR;
        const height = Math.max(HOUR / 4 - 2, ((t.minutes ?? 60) / 60) * HOUR - 2);
        const end = hhmm(Math.min(toMin(t.time!) + (t.minutes ?? 60), 1439));
        const short = height < 36;
        return `<div class="cal-event${t.done ? " done" : ""}${t.urgent ? " urgent" : ""}${short ? " short" : ""}" data-task="${tasks.indexOf(t)}"
          style="top:${top}px;height:${height}px;left:${box.left}px;width:${box.width}px;z-index:${1 + toMin(t.time!) + lane.level / 10}${lane.level ? ";box-shadow:inset 3px 0 0 var(--coral),0 1px 3px rgba(0,0,0,.25)" : ""}"
          title="${esc(t.title)}&#10;${t.time} – ${end}">
          <span>${esc(t.title)}</span><small>${t.time}${short ? "" : "–" + end}</small><i class="cal-resize"></i></div>`;
      }).join("");
      const now = key === today ? `<div class="cal-now" style="top:${((new Date().getHours() * 60 + new Date().getMinutes()) / 60) * HOUR}px"></div>` : "";
      return `<div class="cal-col${key === today ? " today" : ""}" data-day="${key}">${events}${now}</div>`;
    }).join("");
    const heads = list.map((d) => `<div class="cal-day${iso(d) === today ? " today" : ""}">
      <small>${d.toLocaleDateString("ru", { weekday: "short" })}</small><b>${d.getDate()}</b></div>`).join("");
    const allday = list.map((d) => `<div class="cal-allday-col">${tasks.filter((t) => t.date === iso(d) && !t.time)
      .map((t) => `<div class="cal-chip${t.done ? " done" : ""}" data-task="${tasks.indexOf(t)}" title="${esc(t.title)}">${esc(t.title)}</div>`).join("")}</div>`).join("");
    const hours = Array.from({ length: 24 }, (_, h) => `<div class="cal-hour" style="top:${h * HOUR}px"><small>${h}:00</small></div>`).join("");
    return `
      <div class="cal-heads" style="--n:${list.length}"><div></div>${heads}</div>
      <div class="cal-allday" style="--n:${list.length}"><small>весь день</small>${allday}</div>
      <div class="cal-scroll"><div class="cal-grid" style="--n:${list.length};height:${24 * HOUR}px">${hours}<div class="cal-cols">${cols}</div></div></div>`;
  }

  function monthHTML(): string {
    const first = new Date(view.anchor.getFullYear(), view.anchor.getMonth(), 1);
    const start = startOfWeek(first);
    const today = iso(new Date());
    const cells = Array.from({ length: 42 }, (_, i) => {
      const d = addDays(start, i);
      const key = iso(d);
      const list = tasks.filter((t) => t.date === key).sort((a, b) => (a.time ?? "").localeCompare(b.time ?? ""));
      const shown = list.slice(0, list.length > 3 ? 2 : 3);
      return `<button class="cal-cell${d.getMonth() !== first.getMonth() ? " out" : ""}${key === today ? " today" : ""}" data-open-day="${key}">
        <b>${d.getDate()}</b>${shown.map((t) => `<span class="${t.done ? "done" : ""}">${esc(t.title)}</span>`).join("")}
        ${list.length > 3 ? `<em>ещё ${list.length - 2}</em>` : ""}</button>`;
    }).join("");
    const names = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"].map((n) => `<small>${n}</small>`).join("");
    return `<div class="cal-month"><div class="cal-month-names">${names}</div><div class="cal-month-grid">${cells}</div></div>`;
  }

  async function render(keepScroll = false) {
    tasks = await host.tasks();
    const scroll = root.querySelector<HTMLElement>(".cal-scroll")?.scrollTop;
    root.innerHTML = `<div class="cal">
      <header class="cal-head"><div><h1>${title()}</h1><p>задачи с датой из заметок</p></div>${controls()}</header>
      <div class="cal-body">${view.mode === "month" ? monthHTML() : gridHTML()}</div></div>`;
    const sc = root.querySelector<HTMLElement>(".cal-scroll");
    if (sc) sc.scrollTop = keepScroll && scroll !== undefined ? scroll : 8 * HOUR;
  }

  // Новая задача: поле прямо у выбранного времени, Enter — во «Входящие.md».
  function draft(day: string, minutes: number | null, x: number, y: number) {
    root.querySelector(".cal-draft")?.remove();
    const box = document.createElement("div");
    box.className = "cal-draft";
    box.innerHTML = `<small>${new Date(day + "T00:00").toLocaleDateString("ru", { day: "numeric", month: "long" })}${minutes === null ? "" : ", " + hhmm(minutes) + "–" + hhmm(Math.min(minutes + 60, 1439))}</small>
      <input placeholder="новая задача" spellcheck="false">`;
    const r = root.getBoundingClientRect();
    box.style.left = `${Math.min(x - r.left, r.width - 260)}px`;
    box.style.top = `${Math.min(y - r.top, r.height - 90)}px`;
    root.append(box);
    const input = box.querySelector("input")!;
    input.focus();
    let done = false;
    const finish = async (save: boolean) => {
      if (done) return;
      done = true;
      box.remove();
      if (save && input.value.trim()) {
        await host.backend.appendTask(INBOX, input.value, day, minutes === null ? null : hhmm(minutes), minutes === null ? null : 60);
        await render(true);
      }
    };
    input.addEventListener("keydown", (e) => { if (e.key === "Enter") void finish(true); if (e.key === "Escape") void finish(false); });
    input.addEventListener("blur", () => void finish(true));
  }

  // Карточка события (EventCard на маке): название правится прямо тут, «открыть заметку», «удалить».
  function card(task: TaskRow, x: number, y: number) {
    root.querySelector(".cal-draft")?.remove();
    const box = document.createElement("div");
    box.className = "cal-draft";
    const when = `${new Date(task.date + "T12:00").toLocaleDateString("ru", { day: "numeric", month: "long" })}${task.time ? `, ${task.time}–${hhmm(Math.min(toMin(task.time) + (task.minutes ?? 60), 1439))}` : ""}`;
    box.innerHTML = `<input value="${esc(task.title)}" spellcheck="false"><small>${when} · ${esc(task.note.replace(/\.md$/, ""))}</small>
      <div class="cal-card-actions"><button class="pill" data-card="open">открыть заметку</button><button class="pill danger" data-card="delete">удалить</button></div>`;
    const r = root.getBoundingClientRect();
    box.style.left = `${Math.min(x - r.left, r.width - 260)}px`;
    box.style.top = `${Math.min(y - r.top, r.height - 140)}px`;
    root.append(box);
    const input = box.querySelector("input")!;
    input.addEventListener("keydown", async (k) => {
      if (k.key === "Enter" && input.value.trim() && input.value.trim() !== task.title) { await host.backend.setTitle(task, input.value).catch(() => {}); await render(true); }
      if (k.key === "Escape") box.remove();
    });
    box.addEventListener("click", async (e) => {
      const act = (e.target as HTMLElement).closest<HTMLElement>("[data-card]")?.dataset.card;
      if (act === "open") await host.openNote(task.note);
      if (act === "delete") { await host.backend.deleteTask(task).catch(() => {}); await render(true); }
    });
    setTimeout(() => addEventListener("pointerdown", function close(ev) {
      if (!box.contains(ev.target as Node)) { box.remove(); removeEventListener("pointerdown", close, true); }
    }, true), 0);
  }

  root.addEventListener("click", async (e) => {
    const el = e.target as HTMLElement;
    const b = el.closest<HTMLElement>("button");
    if (b?.dataset.cal === "prev") { shift(-1); return render(); }
    if (b?.dataset.cal === "next") { shift(1); return render(); }
    if (b?.dataset.cal === "today") { view.anchor = new Date(); return render(); }
    if (b?.dataset.mode) { view.mode = b.dataset.mode as Mode; return render(); }
    if (b?.dataset.openDay) { view.anchor = new Date(b.dataset.openDay + "T12:00"); view.mode = "day"; return render(); }
    if (b?.dataset.cal === "ics") {
      const saved = await host.backend.exportIcs();
      if (saved) { b.textContent = "сохранено"; setTimeout(() => (b.textContent = ".ics"), 1600); }
      return;
    }
    if (b?.dataset.cal === "new") {
      const day = view.mode === "day" ? iso(view.anchor) : iso(new Date());
      const hour = Math.min(Math.max(new Date().getHours() + 1, 9), 16);
      const r = b.getBoundingClientRect();
      return draft(day, hour * 60, r.left - 220, r.bottom + 8);
    }
  });

  // Нажали на событие: отпустили на месте — открыть заметку, потащили — новое время и день.
  root.addEventListener("pointerdown", (down) => {
    if (down.button !== 0) return;
    const el = down.target as HTMLElement;
    const event = el.closest<HTMLElement>(".cal-event, .cal-chip");
    const col = el.closest<HTMLElement>(".cal-col");
    if (!event) {
      if (!col) return;
      // Пусто — новая задача на этот час (шаг полчаса).
      const r = col.getBoundingClientRect();
      const minutes = Math.min(Math.floor(((down.clientY - r.top) / HOUR) * 2) * 30, 23 * 60);
      draft(col.dataset.day!, minutes, down.clientX, down.clientY);
      down.preventDefault();
      return;
    }
    const task = tasks[Number(event.dataset.task)];
    if (!task) return;
    const timed = event.classList.contains("cal-event");
    // Нижний край — растянуть: меняется только длительность, шаг 15 минут (как на маке).
    if (timed && el.classList.contains("cal-resize")) {
      down.preventDefault();
      const top = event.getBoundingClientRect().top;
      let minutes = task.minutes ?? 60;
      const grow = (e: PointerEvent) => {
        minutes = Math.max(15, Math.round(((e.clientY - top) / HOUR) * 4) * 15);
        event.style.height = `${(minutes / 60) * HOUR - 2}px`;
        event.querySelector("small")!.textContent = `${task.time}–${hhmm(Math.min(toMin(task.time!) + minutes, 1439))}`;
      };
      const stop = async () => {
        removeEventListener("pointermove", grow); removeEventListener("pointerup", stop);
        if (minutes !== (task.minutes ?? 60)) await host.backend.setDue(task, task.date, task.time, minutes).catch(() => {});
        await render(true);
      };
      addEventListener("pointermove", grow); addEventListener("pointerup", stop);
      return;
    }
    const start = { x: down.clientX, y: down.clientY };
    // Где схватили событие — один раз, до сдвига: дальше его рамка уже едет вместе с мышью.
    const grab = timed ? down.clientY - event.getBoundingClientRect().top : 0;
    let moved = false;
    let target: { day: string; minutes: number | null } | null = null;
    const move = (e: PointerEvent) => {
      if (!moved && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 4) return;
      moved = true;
      event.classList.add("moving");
      const over = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>(".cal-col, .cal-allday-col");
      const cols = [...root.querySelectorAll<HTMLElement>(".cal-col")];
      const alls = [...root.querySelectorAll<HTMLElement>(".cal-allday-col")];
      if (!over) return;
      const index = over.classList.contains("cal-col") ? cols.indexOf(over) : alls.indexOf(over);
      const day = cols[index]?.dataset.day;
      if (!day) return;
      if (over.classList.contains("cal-allday-col")) { target = { day, minutes: null }; event.style.transform = ""; return; }
      const r = over.getBoundingClientRect();
      const minutes = Math.max(0, Math.min(Math.round(((e.clientY - grab - r.top) / HOUR) * 4) * 15, 24 * 60 - (task.minutes ?? 60)));
      target = { day, minutes };
      if (timed) {
        const home = event.parentElement!.getBoundingClientRect();
        event.style.transform = `translate(${r.left - home.left}px, ${(minutes / 60) * HOUR - (toMin(task.time!) / 60) * HOUR}px)`;
        event.querySelector("small")!.textContent = hhmm(minutes);
      }
    };
    const up = async () => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
      if (!moved) { card(task, start.x, start.y); return; }
      if (!target) return render(true);
      const time = target.minutes === null ? null : hhmm(target.minutes);
      if (target.day !== task.date || time !== task.time) {
        try {
          await host.backend.setDue(task, target.day, time, time ? task.minutes : null);
        } catch { /* заметку поменяли снаружи — просто перечитаем */ }
      }
      await render(true);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
  });

  root.tabIndex = -1;
  root.addEventListener("keydown", (e) => {
    if ((e.target as HTMLElement).closest("input")) return;
    if (e.key === "ArrowLeft") { shift(-1); void render(); }
    if (e.key === "ArrowRight") { shift(1); void render(); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "t") { e.preventDefault(); view.anchor = new Date(); void render(); }
  });

  void render();
  root.focus({ preventScroll: true });
}
