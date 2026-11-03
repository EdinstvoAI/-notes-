// Связь с Rust-ядром. В окне Tauri — команды из `src-tauri/src/lib.rs`; в обычном браузере
// (`npm run dev`, проверка вида и видео) — демо-хранилище в памяти, чтобы интерфейс жил без Rust.

export interface Note { path: string; title: string; modified: number }
export interface Entry { path: string; name: string }
export interface Folder {
  path: string; name: string;
  folders: Folder[]; notes: Note[]; files: Entry[];
  note_count: number; unreadable: Entry[];
}
export interface TaskRow {
  note: string; line: number; title: string; done: boolean;
  date: string | null; time: string | null; minutes: number | null; urgent: boolean; raw: string;
}

/** Настройки интерфейса — вне хранилища (`prefs.json` в папке настроек приложения). */
export interface Prefs {
  theme?: "system" | "light" | "dark";
  textSize?: number;
  spellcheck?: boolean;
  daily?: boolean;
  /** Как часто сохранять версию в git, минуты. */
  commitMinutes?: number;
  /** Последняя синхронизация с GitHub, мс. */
  lastSync?: number;
  /** Надиктовка: вставлять в поле (Ctrl+V за человека). */
  paste?: boolean;
  /** Хоткей надиктовки, например «ctrl+alt+space». */
  hotkey?: string;
  /** Модель распознавания: «gigaam-v3-punct» или «whisper-small». */
  speechModel?: string;
  /** Язык речи для Whisper: «auto», «ru», «en»… */
  language?: string;
  /** Язык интерфейса: «system», «ru», «en», «sr». */
  uiLanguage?: string;
}

export interface GithubStatus { signed_in: boolean; remote: string | null; unpushed: boolean }
export interface DeviceCode { device_code: string; user_code: string; verification_uri: string; expires_in: number; interval: number }
export type PullResult = { kind: "upToDate" } | { kind: "fastForward" } | { kind: "merged"; conflicts: string[] };

export interface GitStatus { repo: boolean; last_commit: number | null; commits: number; pending: number }

export interface Hit { note: Note; snippet: string; score: number }

export interface Api {
  inTauri: boolean;
  lastVault(): Promise<Folder | null>;
  pickVault(): Promise<Folder | null>;
  tree(): Promise<Folder>;
  read(path: string): Promise<string>;
  write(path: string, text: string): Promise<void>;
  create(folder: string, title: string): Promise<string>;
  rename(path: string, title: string): Promise<string>;
  search(query: string): Promise<Hit[]>;
  tasks(): Promise<TaskRow[]>;
  createFolder(parent: string, name: string): Promise<string>;
  duplicate(path: string): Promise<string>;
  /** null — переносить некуда (та же папка или папка в саму себя). */
  moveInto(path: string, folder: string): Promise<string | null>;
  copyIn(outside: string, folder: string): Promise<string>;
  trash(path: string): Promise<void>;
  untrash(path: string): Promise<void>;
  reveal(path: string): Promise<void>;
  pins(): Promise<string[]>;
  templates(): Promise<string[]>;
  prefs(): Promise<Prefs>;
  setPrefs(prefs: Prefs): Promise<void>;
  vaultPath(): Promise<string | null>;
  gitStatus(): Promise<GitStatus>;
  gitEnable(): Promise<void>;
  gitCommit(): Promise<string | null>;
  githubStatus(): Promise<GithubStatus>;
  githubCode(): Promise<DeviceCode>;
  /** Ждёт подтверждения кода, подключает приватный notes, отправляет историю. Возвращает «логин/notes». */
  githubFinish(code: DeviceCode): Promise<string>;
  githubCancel(): Promise<void>;
  githubSignOut(): Promise<void>;
  githubSync(date: string): Promise<PullResult>;
  openGithub(url: string): Promise<void>;
  /** Сохранить задачи в .ics. false — человек отменил окно сохранения. */
  exportIcs(): Promise<boolean>;
  setDue(task: TaskRow, date: string | null, time: string | null, minutes: number | null): Promise<void>;
  setDone(task: TaskRow, done: boolean): Promise<void>;
  appendTask(path: string, title: string, date: string | null, time: string | null, minutes: number | null): Promise<void>;
  createFromTemplate(template: string, folder: string, date: string, time: string): Promise<string>;
  daily(date: string, time: string): Promise<string>;
  setPins(pins: string[]): Promise<void>;
  os(): Promise<string>;
}

/** Момент сборки .ics в UTC: 20261101T120000Z. */
export const utcStamp = () => new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { open } = await import("@tauri-apps/plugin-dialog");
  return {
    inTauri: true,
    lastVault: () => invoke<Folder | null>("last_vault"),
    async pickVault() {
      const path = await open({ directory: true, multiple: false, title: "Папка с заметками" });
      return typeof path === "string" ? invoke<Folder>("open_vault", { path }) : null;
    },
    tree: () => invoke<Folder>("tree"),
    read: (path) => invoke<string>("read_note", { path }),
    write: (path, text) => invoke<void>("write_note", { path, text }),
    create: (folder, title) => invoke<string>("create_note", { folder, title }),
    rename: (path, title) => invoke<string>("rename_note", { path, title }),
    search: (query) => invoke<Hit[]>("search", { query }),
    tasks: () => invoke<TaskRow[]>("tasks"),
    createFolder: (parent, name) => invoke<string>("create_folder", { parent, name }),
    duplicate: (path) => invoke<string>("duplicate", { path }),
    moveInto: (path, folder) => invoke<string | null>("move_into", { path, folder }),
    copyIn: (outside, folder) => invoke<string>("copy_in", { outside, folder }),
    trash: (path) => invoke<void>("trash", { path }),
    untrash: (path) => invoke<void>("untrash", { path }),
    reveal: (path) => invoke<void>("reveal", { path }),
    pins: () => invoke<string[]>("pins"),
    templates: () => invoke<string[]>("templates"),
    prefs: async () => JSON.parse(await invoke<string>("prefs")) as Prefs,
    setPrefs: (prefs) => invoke<void>("set_prefs", { json: JSON.stringify(prefs) }),
    vaultPath: () => invoke<string | null>("vault_path"),
    gitStatus: () => invoke<GitStatus>("git_status"),
    gitEnable: () => invoke<void>("git_enable"),
    gitCommit: () => invoke<string | null>("git_commit"),
    githubStatus: () => invoke<GithubStatus>("github_status"),
    githubCode: () => invoke<DeviceCode>("github_code"),
    githubFinish: (code) => invoke<string>("github_finish", { code }),
    githubCancel: () => invoke<void>("github_cancel"),
    githubSignOut: () => invoke<void>("github_sign_out"),
    githubSync: (date) => invoke<PullResult>("github_sync", { date }),
    openGithub: (url) => invoke<void>("open_github", { url }),
    async exportIcs() {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const path = await save({ defaultPath: "notes.ics", filters: [{ name: "iCalendar", extensions: ["ics"] }], title: "Задачи для календаря" });
      if (!path) return false;
      await invoke("export_ics", { path, stamp: utcStamp() });
      return true;
    },
    setDue: (t, date, time, minutes) => invoke<void>("set_due", { path: t.note, line: t.line, raw: t.raw, date, time, minutes }),
    setDone: (t, done) => invoke<void>("set_done", { path: t.note, line: t.line, raw: t.raw, done }),
    appendTask: (path, title, date, time, minutes) => invoke<void>("append_task", { path, title, date, time, minutes }),
    createFromTemplate: (template, folder, date, time) => invoke<string>("create_from_template", { template, folder, date, time }),
    daily: (date, time) => invoke<string>("daily", { date, time }),
    setPins: (pins) => invoke<void>("set_pins", { pins }),
    os: () => invoke<string>("os"),
  };
}

// ——— демо-хранилище ———

// Демо-задачи — вокруг сегодняшнего дня, чтобы неделя в календаре не пустовала.
function dayOffset(n: number) {
  const d = new Date();
  d.setDate(d.getDate() + n);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

const demo: Record<string, string> = {
  "Входящие.md": `- [ ] купить хлеб 📅 ${dayOffset(1)}\n- [ ] позвонить Ане !! 📅 ${dayOffset(2)} 15:00\n- [ ] созвон с командой 📅 ${dayOffset(0)} 11:00–12:30\n- [ ] спортзал 📅 ${dayOffset(0)} 19:00–20:30\n- [x] выпустить 1.0 📅 ${dayOffset(-1)} 10:00\n`,
  "проекты/план приложения.md":
    "# Что делаем\n\nЗаметки — обычные файлы Markdown в папке. История — в git, на GitHub по желанию.\n\n## Windows\n\n- [x] ядро на Rust\n- [ ] стекло как на маке 📅 2026-11-05\n- [ ] надиктовка по хоткею\n\nСинхронизация идёт сама: раз в час коммит, раз в день — на GitHub.\n",
  "проекты/лендинг.md": "# Лендинг\n\nОдин экран, кнопка «скачать», три фишки.\n",
  "проекты/дизайн/шапка.md": "# Шапка\n\nКапсулы 48, скругление 18, стекло 20 %.\n",
  "дневник/2026-11-01.md": "# 1 ноября\n\nВыпустили 1.0.0.\n",
  "идеи.md": "- рисунки внутри заметки\n- поиск по смыслу точнее\n",
  "шаблоны/встреча.md": "# {{название}}\n\n{{дата}} {{время}}\n\n## Кто\n\n## Решили\n\n- [ ] \n",
  "шаблоны/день.md": "# {{дата}}\n\n## Главное\n\n- [ ] \n",
};
const demoTime = Math.floor(Date.now() / 1000) - 600;
const demoFolders = new Set<string>();
const demoDate = (date: string | null, time: string | null, minutes: number | null) => {
  if (!date) return "";
  if (!time) return ` 📅 ${date}`;
  if (!minutes) return ` 📅 ${date} ${time}`;
  const [h, m] = time.split(":").map(Number);
  const end = Math.min(h * 60 + m + minutes, 23 * 60 + 59);
  return ` 📅 ${date} ${time}–${String(Math.floor(end / 60)).padStart(2, "0")}:${String(end % 60).padStart(2, "0")}`;
};
let demoPins: string[] = [];
const demoTrash: Record<string, string> = {};
const demoGithub = { signed: false, synced: 0 };
const demoGit = { repo: true, last_commit: Math.floor(Date.now() / 1000) - 12 * 60 as number | null, commits: 42, pending: 0 };
const demoFree = (want: (n: number) => string, taken: (p: string) => boolean) => {
  let n = 1;
  while (taken(want(n))) n++;
  return want(n);
};

function demoTree(): Folder {
  const root: Folder = { path: "", name: "заметки", folders: [], notes: [], files: [], note_count: 0, unreadable: [] };
  const ensure = (path: string): Folder => {
    if (!path) return root;
    const parts = path.split("/");
    let node = root;
    parts.forEach((part, i) => {
      const p = parts.slice(0, i + 1).join("/");
      let child = node.folders.find((f) => f.path === p);
      if (!child) {
        child = { path: p, name: part, folders: [], notes: [], files: [], note_count: 0, unreadable: [] };
        node.folders.push(child);
      }
      node = child;
    });
    return node;
  };
  demoFolders.forEach((f) => ensure(f));
  for (const path of Object.keys(demo)) {
    const slash = path.lastIndexOf("/");
    const folder = ensure(slash < 0 ? "" : path.slice(0, slash));
    folder.notes.push({ path, title: path.slice(slash + 1, -3), modified: demoTime });
  }
  const sort = (f: Folder): number => {
    const cmp = (a: string, b: string) => a.localeCompare(b, "ru", { numeric: true, sensitivity: "base" });
    f.folders.sort((a, b) => cmp(a.name, b.name));
    f.notes.sort((a, b) => cmp(a.title, b.title));
    f.note_count = f.notes.length + f.folders.reduce((n, c) => n + sort(c), 0);
    return f.note_count;
  };
  sort(root);
  return root;
}

const demoApi: Api = {
  inTauri: false,
  lastVault: async () => demoTree(),
  pickVault: async () => demoTree(),
  tree: async () => demoTree(),
  read: async (path) => demo[path] ?? "",
  write: async (path, text) => { demo[path] = text; },
  async create(folder, title) {
    let n = 1;
    const name = () => `${folder ? folder + "/" : ""}${title}${n === 1 ? "" : " " + n}.md`;
    while (name() in demo) n++;
    demo[name()] = "";
    return name();
  },
  async rename(path, title) {
    const folder = path.includes("/") ? path.slice(0, path.lastIndexOf("/") + 1) : "";
    const to = `${folder}${title.trim()}.md`;
    if (to === path || !title.trim()) return path;
    if (to in demo) throw new Error("name taken");
    demo[to] = demo[path]; delete demo[path];
    return to;
  },
  // Как `core/src/search.rs`: все слова, ё = е, название весит больше.
  async search(query) {
    const norm = (s: string) => s.toLowerCase().replaceAll("ё", "е");
    const words = norm(query).split(/\s+/).filter(Boolean);
    if (!words.length) return [];
    const hits: Hit[] = [];
    for (const [path, text] of Object.entries(demo)) {
      const title = path.slice(path.lastIndexOf("/") + 1, -3);
      const t = norm(title), b = norm(text);
      if (!words.every((w) => t.includes(w) || b.includes(w))) continue;
      const lines = text.split("\n").map((l) => l.trim());
      const line = lines.find((l) => norm(l).includes(words[0]) && !l.startsWith("#")) ?? lines.find(Boolean) ?? "";
      const snippet = line.replace(/^[#\s\-—*]+/, "").replace(/^\[[ xX]\] /, "").split("📅")[0].trim();
      const score = words.filter((w) => t.includes(w)).length * 3 + Math.min(10, words.reduce((n, w) => n + b.split(w).length - 1, 0));
      hits.push({ note: { path, title, modified: demoTime }, snippet, score });
    }
    return hits.sort((a, b) => b.score - a.score).slice(0, 20);
  },
  // Упрощённый разбор, только для демо; настоящий — `core/src/tasks.rs`.
  async createFolder(parent, name) {
    const base = `${parent ? parent + "/" : ""}${name.trim()}`;
    const path = demoFree((n) => (n === 1 ? base : `${base} ${n}`), (p) => demoFolders.has(p) || Object.keys(demo).some((k) => k.startsWith(p + "/")));
    demoFolders.add(path);
    return path;
  },
  async duplicate(path) {
    const base = path.replace(/\.md$/, " копия");
    const to = demoFree((n) => `${base}${n === 1 ? "" : " " + n}.md`, (p) => p in demo);
    demo[to] = demo[path] ?? "";
    return to;
  },
  async moveInto(path, folder) {
    const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
    if (parent === folder || folder === path || folder.startsWith(path + "/")) return null;
    const name = path.slice(path.lastIndexOf("/") + 1);
    const moved: Record<string, string> = {};
    for (const k of Object.keys(demo)) {
      if (k === path || k.startsWith(path + "/")) moved[k] = `${folder ? folder + "/" : ""}${name}${k.slice(path.length)}`;
    }
    for (const [from, to] of Object.entries(moved)) { demo[to] = demo[from]; delete demo[from]; }
    if (demoFolders.delete(path)) demoFolders.add(`${folder ? folder + "/" : ""}${name}`);
    return `${folder ? folder + "/" : ""}${name}`;
  },
  copyIn: async () => { throw new Error("no files outside in the browser"); },
  async trash(path) {
    for (const k of Object.keys(demo)) if (k === path || k.startsWith(path + "/")) { demoTrash[k] = demo[k]; delete demo[k]; }
    demoFolders.delete(path);
  },
  async untrash(path) {
    for (const k of Object.keys(demoTrash)) if (k === path || k.startsWith(path + "/")) { demo[k] = demoTrash[k]; delete demoTrash[k]; }
  },
  reveal: async () => {},
  pins: async () => demoPins.filter((p) => p in demo),
  async setDue(task, date, time, minutes) {
    const lines = (demo[task.note] ?? "").split("\n");
    if (lines[task.line] !== task.raw) throw new Error("the note changed on disk");
    lines[task.line] = lines[task.line].split("📅")[0].trimEnd() + demoDate(date, time, minutes);
    demo[task.note] = lines.join("\n");
  },
  async setDone(task, done) {
    const lines = (demo[task.note] ?? "").split("\n");
    if (lines[task.line] !== task.raw) throw new Error("the note changed on disk");
    lines[task.line] = lines[task.line].replace(/\[[ xX]\]/, done ? "[x]" : "[ ]");
    demo[task.note] = lines.join("\n");
  },
  async appendTask(path, title, date, time, minutes) {
    if (!title.trim()) return;
    let text = demo[path] ?? "";
    if (text && !text.endsWith("\n")) text += "\n";
    demo[path] = text + `- [ ] ${title.trim()}${demoDate(date, time, minutes)}\n`;
  },
  // В браузере файла нет — отдаём как загрузку; формат тот же, что собирает `core/src/ics.rs`.
  async exportIcs() {
    const rows = await demoApi.tasks();
    const lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//notes//tasks//RU"];
    for (const t of rows.filter((r) => r.date)) {
      const d = t.date!.replaceAll("-", "");
      lines.push("BEGIN:VEVENT", `UID:${t.note}#${t.line}@notes`, `DTSTAMP:${utcStamp()}`,
        t.time ? `DTSTART:${d}T${t.time.replace(":", "")}00` : `DTSTART;VALUE=DATE:${d}`, `SUMMARY:${t.title}`, "END:VEVENT");
    }
    lines.push("END:VCALENDAR");
    const a = Object.assign(document.createElement("a"), { href: URL.createObjectURL(new Blob([lines.join("\r\n")], { type: "text/calendar" })), download: "notes.ics" });
    a.click();
    return true;
  },
  async prefs() {
    try { return JSON.parse(localStorage.getItem("notes-prefs") ?? "{}") as Prefs; } catch { return {}; }
  },
  async setPrefs(prefs) {
    try { localStorage.setItem("notes-prefs", JSON.stringify(prefs)); } catch { /* приватное окно */ }
  },
  gitStatus: async () => ({ ...demoGit }),
  async gitEnable() { demoGit.repo = true; demoGit.commits = 1; demoGit.last_commit = Math.floor(Date.now() / 1000); },
  async gitCommit() {
    if (!demoGit.repo) return null;
    demoGit.commits++; demoGit.last_commit = Math.floor(Date.now() / 1000);
    return "правки: план приложения.md";
  },
  githubStatus: async () => ({ signed_in: demoGithub.signed, remote: demoGithub.signed ? "https://github.com/makar/notes.git" : null, unpushed: false }),
  githubCode: async () => ({ device_code: "demo", user_code: "WDJB-MJHT", verification_uri: "https://github.com/login/device", expires_in: 900, interval: 5 }),
  async githubFinish() { await new Promise((r) => setTimeout(r, 2500)); demoGithub.signed = true; return "makar/notes"; },
  githubCancel: async () => {},
  async githubSignOut() { demoGithub.signed = false; },
  async githubSync() { demoGit.last_commit = Math.floor(Date.now() / 1000); demoGithub.synced = Date.now(); return { kind: "upToDate" }; },
  openGithub: async () => {},
  vaultPath: async () => "C:\\Users\\makar\\Documents\\заметки (демо)",
  templates: async () => Object.keys(demo).filter((k) => k.startsWith("шаблоны/")).map((k) => k.slice(8, -3)).sort(),
  async createFromTemplate(template, folder, date, time) {
    const path = await demoApi.create(folder, template);
    const title = path.slice(path.lastIndexOf("/") + 1, -3);
    demo[path] = demo[`шаблоны/${template}.md`].replaceAll("{{дата}}", date).replaceAll("{{время}}", time).replaceAll("{{название}}", title);
    return path;
  },
  async daily(date, time) {
    const path = `дневник/${date}.md`;
    if (!(path in demo)) demo[path] = (demo["шаблоны/день.md"] ?? "# {{дата}}\n\n").replaceAll("{{дата}}", date).replaceAll("{{время}}", time).replaceAll("{{название}}", date);
    return path;
  },
  setPins: async (pins) => { demoPins = pins; },
  tasks: async () =>
    Object.entries(demo).flatMap(([note, text]) =>
      text.split("\n").flatMap((line, i) => {
        const m = /^\s*[-*] \[([ xX])\] (.+?)(?: 📅 (\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2})(?:–(\d{2}:\d{2}))?)?.*)?$/u.exec(line);
        if (!m) return [];
        const urgent = m[2].includes("!!");
        const toMin = (s: string) => Number(s.slice(0, 2)) * 60 + Number(s.slice(3));
        const minutes = m[4] && m[5] && toMin(m[5]) > toMin(m[4]) ? toMin(m[5]) - toMin(m[4]) : null;
        return [{ note, line: i, title: m[2].replace("!!", "").trim(), done: m[1] !== " ",
          date: m[3] ?? null, time: m[4] ?? null, minutes, urgent, raw: line }];
      })),
  os: async () => "browser",
};

export const api: Promise<Api> = inTauri ? tauriApi() : Promise.resolve(demoApi);
