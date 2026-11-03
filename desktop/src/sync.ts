// Раздел «синхронизация» — порт `SyncScreen.swift` (03.11.2026): версии хранилища из git и откат,
// «как git, но графически». Работает и без GitHub. В панели — «граф» и версии по дням линией метро;
// справа — версия: файлы (клик — каким был тогда, «вернуть этот файл»), «отменить эти правки»,
// «вернуть всё к этой версии». «граф» — доска на весь лист, клик по станции — карточка рядом.
// Откат — новая версия сверху, поэтому его тоже можно отменить.

import { invoke } from "@tauri-apps/api/core";
import { icon } from "./icons";

interface Change { path: string; kind: "added" | "modified" | "deleted" }
interface Version { id: string; message: string; date: number; parents: string[]; files: Change[] }

export interface SyncHost {
  inTauri: boolean;
  /** Почему не отправилось на GitHub — или null. */
  error(): string | null;
  pushNow(): Promise<void>;
  hasRemote(): Promise<boolean>;
  lastPush(): number | null;
  /** Файлы поменялись на диске — перечитать дерево и открытую заметку. */
  reload(): Promise<void>;
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const day = (d: number) => new Date(d * 1000).toLocaleDateString("ru", { day: "numeric", month: "long" });
const time = (d: number) => new Date(d * 1000).toLocaleTimeString("ru", { hour: "2-digit", minute: "2-digit" });
const full = (d: number) => `${day(d)} ${time(d)}`;
const files = (n: number) => `${n} ${n % 10 === 1 && n % 100 !== 11 ? "файл" : [2, 3, 4].includes(n % 10) && ![12, 13, 14].includes(n % 100) ? "файла" : "файлов"}`;
const KIND = { added: "новый", modified: "изменён", deleted: "удалён" };

const st = { commits: [] as Version[], selected: null as string | null, board: false, card: null as string | null,
  open: null as string | null, preview: null as string | null, error: null as string | null, busy: false, repo: true, remote: false, step: 64 };

export async function mountSync(root: HTMLElement, sidebar: HTMLElement, host: SyncHost) {
  async function load() {
    try {
      st.commits = host.inTauri ? await invoke<Version[]>("git_history") : [];
      st.repo = true;
    } catch { st.commits = []; st.repo = false; }
    st.remote = await host.hasRemote().catch(() => false);
    if (!st.commits.some((c) => c.id === st.selected)) st.selected = st.commits[0]?.id ?? null;
    paint();
  }

  const when = (c: Version) => {
    const pushed = host.lastPush() !== null && c.date * 1000 <= host.lastPush()!;
    return `${full(c.date)} · ${st.remote ? (pushed ? "на github" : "ещё не на github") : "только на этом компьютере"}`;
  };
  const older = (id: string) => { const i = st.commits.findIndex((c) => c.id === id); return st.commits[i + 1]?.id ?? id; };

  function paintSidebar() {
    let html = `<button class="row${st.board ? " selected" : ""}" style="padding-left:8px;margin-bottom:12px" data-sync="board">${icon("git")}<span>граф</span></button>`;
    st.commits.forEach((c, i) => {
      const last = i === st.commits.length - 1;
      if (i === 0 || day(c.date) !== day(st.commits[i - 1].date))
        html += `<div class="metro-day${i ? " top" : ""}"><i class="metro"></i><small>${day(c.date)}</small></div>`;
      html += `<button class="row metro-row${st.selected === c.id ? " selected" : ""}" data-version="${c.id}">
        <i class="metro top${last ? "" : " bottom"}${st.selected === c.id ? " stop on" : " stop"}"></i>
        <small>${time(c.date)}</small><span>${esc(c.message)}</span></button>`;
    });
    sidebar.innerHTML = html;
  }

  function details(c: Version, width: number): string {
    const files = c.files.map((f) => {
      const source = f.kind === "deleted" ? older(c.id) : c.id;
      const open = st.open === f.path;
      return `<button class="version-file" style="width:${width}px" data-file="${esc(f.path)}" data-source="${source}">
          <div><b>${esc(f.path)}</b><small>${KIND[f.kind]}</small></div>
          ${open ? `<span class="pill" data-restore-file="${esc(f.path)}" data-source="${source}">вернуть этот файл</span>` : ""}</button>
        ${open ? `<pre class="version-preview${st.preview === null ? " empty" : ""}" style="width:${width}px">${esc(st.preview ?? "в этой версии файла нет")}</pre>` : ""}`;
    }).join("");
    return `<div class="version" style="width:${width}px">${files}
      ${st.error ? `<p class="sync-error">${esc(st.error)}</p>` : ""}
      <div class="version-actions"${st.busy ? " inert" : ""}>
        <button class="pill" data-revert="${c.id}">отменить эти правки</button>
        ${c.id !== st.commits[0]?.id ? `<button class="pill" data-restore="${c.id}">вернуть всё к этой версии</button>` : ""}
      </div>
      <small class="version-note">откат — новая версия сверху: его тоже можно отменить</small></div>`;
  }

  function board(): string {
    const gap = st.step;
    return `<div class="sync-board">${st.commits.map((c, i) => {
      const on = st.selected === c.id;
      const newDay = i === 0 || day(c.date) !== day(st.commits[i - 1].date);
      return `<div class="board-row${on ? " on" : ""}" style="height:${gap}px" data-station="${c.id}">
        <small class="board-day">${newDay ? new Date(c.date * 1000).toLocaleDateString("ru", { day: "numeric", month: "short" }) : ""}</small>
        <div class="board-line${i ? " top" : ""}${i < st.commits.length - 1 ? " bottom" : ""}"><i></i></div>
        <div class="board-text"><span>${esc(c.message)}</span><small>${time(c.date)} · ${files(c.files.length)}</small></div>
        ${st.card === c.id ? `<div class="board-card"><h3>${esc(c.message)}</h3><small>${when(c)}</small>${details(c, 360)}</div>` : ""}
      </div>`;
    }).join("")}</div>`;
  }

  function paint() {
    paintSidebar();
    const head = (t: string, s: string) => `<h1>${esc(t)}</h1><p class="settings-sub">${esc(s)}</p>`;
    const problem = host.error();
    const c = st.commits.find((v) => v.id === st.selected);
    root.innerHTML = !st.repo ? `<div class="settings">${head("истории нет", "включи git в настройках → синхронизация")}</div>`
      : !st.commits.length ? `<div class="settings">${head("версий пока нет", "приложение сохраняет версию по расписанию")}</div>`
      : st.board ? board()
      : c ? `<div class="settings">
          ${problem ? `<div class="sync-problem"><span>${esc(problem)}</span><button class="pill" data-push>отправить сейчас</button></div>` : ""}
          ${head(c.message, when(c))}<div class="settings-rows">${details(c, 640)}</div></div>` : "";
    if (st.board && st.card) root.querySelector(".board-row.on")?.scrollIntoView({ block: "center", behavior: "smooth" });
  }

  async function run(action: () => Promise<unknown>) {
    st.busy = true; st.error = null; paint();
    try { await action(); } catch (e) { st.error = String(e); }
    st.busy = false; st.open = null;
    const keep = st.error;
    await load();
    st.error = keep;
    if (!keep) st.selected = st.commits[0]?.id ?? null;
    paint();
    await host.reload();
  }

  const confirm = async (title: string, text: string, ok: string) => {
    if (!host.inTauri) return window.confirm(`${title}\n${text}`);
    const { ask } = await import("@tauri-apps/plugin-dialog");
    return ask(text, { title, kind: "warning", okLabel: ok, cancelLabel: "не надо" });
  };

  root.onclick = async (e) => {
    const t = e.target as HTMLElement;
    const b = t.closest<HTMLElement>("[data-push],[data-restore-file],[data-file],[data-revert],[data-restore],[data-station]");
    if (!b) return;
    const c = st.commits.find((v) => v.id === (b.dataset.revert ?? b.dataset.restore));
    if (b.dataset.push !== undefined) { await host.pushNow(); paint(); }
    else if (b.dataset.restoreFile) { const [path, id] = [b.dataset.restoreFile, b.dataset.source!]; await run(() => invoke("git_restore_file", { path, id })); }
    else if (b.dataset.file) {
      const path = b.dataset.file;
      if (st.open === path) st.open = null;
      else { st.preview = await invoke<string | null>("git_file_at", { path, id: b.dataset.source }).catch(() => null); st.open = path; }
      paint();
    } else if (b.dataset.revert && c) {
      if (await confirm(`отменить «${c.message}»?`, "уйдут только правки этой версии, всё, что было после, останется", "отменить правки"))
        await run(() => invoke("git_revert", { id: c.id }));
    } else if (b.dataset.restore && c) {
      if (await confirm(`вернуть всё к версии от ${full(c.date)}?`, "все заметки станут как тогда; текущее состояние останется в истории", "вернуть"))
        await run(() => invoke("git_restore", { id: c.id, stamp: full(c.date) }));
    } else if (b.dataset.station && !t.closest(".board-card")) {
      st.selected = b.dataset.station; st.card = st.card === b.dataset.station ? null : b.dataset.station; st.open = null; paint();
    }
  };
  sidebar.onclick = (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-sync],[data-version]");
    if (!b || !root.isConnected) return;
    if (b.dataset.sync) { st.board = !st.board; st.card = null; }
    else { st.selected = b.dataset.version!; st.open = null; if (st.board) st.card = st.selected; }
    paint();
  };
  // Масштаб доски: Ctrl + колесо или Ctrl+= / Ctrl+−, шаг между станциями 36…140 (щипок на маке).
  const zoom = (k: number) => { st.step = Math.min(140, Math.max(36, st.step * k)); if (st.board) paint(); };
  root.onwheel = (e) => { if (st.board && e.ctrlKey) { e.preventDefault(); zoom(e.deltaY < 0 ? 1.1 : 1 / 1.1); } };
  const keys = (e: KeyboardEvent) => {
    if (!root.isConnected) return document.removeEventListener("keydown", keys);
    if (st.board && e.ctrlKey && (e.key === "=" || e.key === "+")) { e.preventDefault(); zoom(1.25); }
    if (st.board && e.ctrlKey && e.key === "-") { e.preventDefault(); zoom(1 / 1.25); }
  };
  document.addEventListener("keydown", keys);

  st.open = null;
  await load();
}
