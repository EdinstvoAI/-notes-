// Экран «видео» — порт `VideoScreen.swift`: зона «перетащи файл», список расшифровок слева, расшифровка
// с временем и спикерами справа; «сохранить как заметку», копировать, .srt, удалить.
// Распознаёт тот же GigaAM, что и надиктовка, на этом компьютере.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { icon } from "./icons";
import { showMenu } from "./menu";
import { DROPZONE, shader } from "./gl";

interface Item { id: string; title: string; source: string; added: string; duration: number; words: number | null; error?: string | null; folder?: string | null }
interface Segment { start: number; end: number; text: string; speaker?: number }
interface Transcript { segments: Segment[]; names: Record<string, string> }

export interface VideoHost { openNote(path: string): Promise<void>; reload(): Promise<void>; onCrumb(t: string): void; inTauri: boolean }

const TYPES = ["mp4", "mov", "m4v", "m4a", "m4b", "mp3", "wav", "aac", "flac", "3gp", "aif"];
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const clock = (t: number) => {
  const s = Math.floor(t), h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  const p = (n: number) => String(n).padStart(2, "0");
  return h ? `${h}:${p(m)}:${p(x)}` : `${p(m)}:${p(x)}`;
};
const nowStamp = () => { const d = new Date(), p = (n: number) => String(n).padStart(2, "0"); return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`; };

let open: string | null = null;

/** Открыть расшифровку (из поиска). */
export function selectVideo(id: string) { open = id; }

/** Зона «перетащите» — шейдер DropZone.metal: стеклянные планки, за ними плывут тёплые пятна. */
function animateZone() {
  const zone = document.querySelector<HTMLElement>(".drop-zone");
  const canvas = zone?.querySelector<HTMLCanvasElement>(".drop-canvas");
  if (!zone || !canvas) return;
  const draw = shader(canvas, DROPZONE);
  if (!draw) return;
  let on = 0, target = 0;
  zone.addEventListener("pointerenter", () => (target = 1));
  zone.addEventListener("pointerleave", () => (target = 0));
  const start = performance.now();
  const dark = () => document.documentElement.dataset.theme === "dark" || (!document.documentElement.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
  const frame = (now: number) => {
    if (!canvas.isConnected) return;
    on += (target - on) * 0.08;
    draw({ t: (now - start) / 1000, on, dark: dark() ? 1 : 0, scale: devicePixelRatio || 1 });
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

/** Добавить файлы (из окна выбора или перетащенные из Проводника). */
export async function addFiles(paths: string[]) {
  for (const path of paths) {
    if (TYPES.includes(path.split(".").pop()!.toLowerCase())) open = (await invoke<Item>("video_add", { path, added: nowStamp() })).id;
  }
}

export async function mountVideo(root: HTMLElement, sidebar: HTMLElement, host: VideoHost) {
  if (!host.inTauri) {
    root.innerHTML = `<div class="placeholder"><h1>видео</h1><p>Расшифровка работает в окне приложения: в браузере нет модели.</p></div>`;
    return;
  }
  let items: Item[] = [];

  // Папки расшифровок, как на маке: щелчок по папке — только её расшифровки, правый клик по расшифровке —
  // «в папку». Сохранённая заметка ложится в «расшифровки/папка/».
  let folderFilter: string | null = null;
  // Фильтры слева — как VideoModel.Filter на маке; очереди у ядра нет, поэтому «в очереди» пусто.
  type Filter = "all" | "working" | "queued" | "done";
  let filter: Filter = "all";
  const FILTERS: [Filter, string][] = [["all", "все расшифровки"], ["working", "в работе"], ["queued", "в очереди"], ["done", "готово"]];
  const working = (i: Item) => i.words === null && !i.error;
  const pass = (i: Item, f: Filter) => f === "all" || (f === "working" ? working(i) : f === "done" ? i.words !== null && !i.error : false);
  const folderList = () => [...new Set(items.map((i) => i.folder).filter((f): f is string => !!f))].sort();
  const plural = (n: number, one: string, few: string, many: string) => n % 10 === 1 && n % 100 !== 11 ? one : [2, 3, 4].includes(n % 10) && ![12, 13, 14].includes(n % 100) ? few : many;
  let model = "GigaAM v3";
  const renderSidebar = () => {
    sidebar.innerHTML = FILTERS.map(([f, title]) => `<button class="row${filter === f && !folderFilter ? " selected" : ""}" style="padding-left:8px" data-vfilter="${f}">${icon(f === "done" ? "taskDone" : "video")}<span>${title}</span><small class="count">${items.filter((i) => pass(i, f)).length}</small></button>`).join("")
      + folderList().map((f) => `<button class="row${folderFilter === f ? " selected" : ""}" style="padding-left:8px" data-vfolder="${esc(f)}">${icon("folder")}<span>${esc(f)}</span><small class="count">${items.filter((i) => i.folder === f).length}</small></button>`).join("")
      + `<button class="row quiet-row" style="padding-left:8px" data-vnew>${icon("plus")}<span>новая папка</span></button>`;
  };

  const clockOf = (d: number) => d > 0 ? " · " + clock(d) : "";
  const meta = (i: Item) => i.error ? "файл недоступен, добавь заново"
    : working(i) ? i.source.split(".").pop()!.toLowerCase() + clockOf(i.duration)
    : `${i.added.slice(0, 10).split("-").reverse().slice(0, 2).join(".")}${clockOf(i.duration)} · ${i.words} ${plural(i.words ?? 0, "слово", "слова", "слов")}`;
  const videoRow = (i: Item) => `<div class="video-row" data-vrow="${i.id}">${icon("video")}<div><b>${esc(i.title)}</b><small>${esc(meta(i))}</small></div>
    ${working(i) ? `<span class="video-bar"><i></i></span>`
      : i.error ? `<button class="quiet-link" data-vdel="${i.id}">убрать</button>`
      : `<button class="quiet-link" data-vid="${i.id}">открыть</button><button class="quiet-link" data-vdel="${i.id}">удалить</button>`}</div>`;
  const group = (title: string, list: Item[]) => list.length ? `<div class="video-group">${title}</div>${list.map(videoRow).join("")}` : "";
  const dropZone = () => {
    const visible = items.filter((i) => pass(i, filter) && (!folderFilter || i.folder === folderFilter));
    const folders = folderList();
    return `<div class="video">
    <h1>видео</h1><p class="video-sub">${items.length} ${plural(items.length, "расшифровка", "расшифровки", "расшифровок")} | распознаёт ${model}</p>
    <button class="drop-zone" data-pick>
      <canvas class="drop-canvas"></canvas>
      ${icon("plus", 24)}<b>перетащи видео или аудио</b><small>или нажми, чтобы выбрать · mp4, mov, mp3, wav, flac и другие</small>
    </button>
    ${!folderFilter && folders.length ? `<div class="video-group">папки</div>${folders.map((f) => `<button class="video-folder" data-vfolder="${esc(f)}">${icon("folder")}<span>${esc(f)}</span><small>${items.filter((i) => i.folder === f).length}</small></button>`).join("")}` : ""}
    ${group("в работе", visible.filter(working))}${group("готово", visible.filter((i) => i.words !== null && !i.error))}${group("не получилось", visible.filter((i) => !!i.error))}
    </div>`;
  };

  async function renderItem(item: Item) {
    if (item.words === null && !item.error) {
      root.innerHTML = `<div class="video"><h1>${esc(item.title)}</h1><p class="video-sub">распознаём… это идёт на этом компьютере</p><div class="video-progress"><i></i></div></div>`;
      return;
    }
    if (item.error) {
      root.innerHTML = `<div class="video"><h1>${esc(item.title)}</h1><p class="video-sub error">${esc(item.error)}</p>
        <div class="video-actions"><button class="pill" data-del>удалить</button></div></div>`;
      return;
    }
    const t = await invoke<Transcript | null>("video_get", { id: item.id });
    if (!t) return;
    let previous: number | undefined;
    const lines = t.segments.map((s) => {
      const who = s.speaker !== undefined && s.speaker !== previous
        ? `<button class="speaker" data-speaker="${s.speaker}">${esc(t.names[s.speaker] ?? `спикер ${s.speaker + 1}`)}</button>` : "";
      previous = s.speaker ?? previous;
      return `<div class="line" data-line="${t.segments.indexOf(s)}">${who}<time>${clock(s.start)}</time><p>${esc(s.text)}</p></div>`;
    }).join("");
    root.innerHTML = `<div class="video">
      <h1>${esc(item.title)}</h1>
      <p class="video-sub">${clock(item.duration)} · ${item.words} слов · ${esc(item.source)}</p>
      <div class="video-actions">
        <button class="pill primary-pill" data-note>сохранить как заметку</button>
        <button class="pill" data-copy>копировать</button><button class="pill" data-srt>.srt</button><button class="pill" data-del>удалить</button>
      </div>
      <div class="caption-line">расшифровка</div>
      <div class="transcript">${lines}</div></div>`;
  }

  async function render() {
    items = await invoke<Item[]>("video_list");
    const id = (JSON.parse(await invoke<string>("prefs").catch(() => "{}")) as { speechModel?: string }).speechModel ?? "gigaam-v3-punct";
    model = id.includes("turbo") ? "Whisper large-v3 turbo" : id.includes("small") ? "Whisper small" : "GigaAM v3";
    renderSidebar();
    const item = items.find((i) => i.id === open);
    host.onCrumb(item ? `видео / ${item.title}` : "видео");
    if (item) await renderItem(item); else { root.innerHTML = dropZone(); animateZone(); }
  }

  root.addEventListener("click", async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    const item = items.find((i) => i.id === open);
    if (b.dataset.vid) { open = b.dataset.vid; await render(); return; }
    if (b.dataset.vfolder) { folderFilter = b.dataset.vfolder; await render(); return; }
    if (b.dataset.vdel) { await invoke("video_delete", { id: b.dataset.vdel }); await render(); return; }
    if (b.dataset.pick !== undefined) {
      const { open: pick } = await import("@tauri-apps/plugin-dialog");
      const files = await pick({ multiple: true, filters: [{ name: "видео и аудио", extensions: TYPES }] });
      if (files) { await addFiles(Array.isArray(files) ? files : [files]); await render(); }
    } else if (item && b.dataset.note !== undefined) {
      const path = await invoke<string>("video_save_note", { id: item.id });
      await host.reload();
      await host.openNote(path);
    } else if (item && b.dataset.copy !== undefined) {
      const t = await invoke<Transcript | null>("video_get", { id: item.id });
      await navigator.clipboard.writeText(t?.segments.map((s) => s.text).join(" ") ?? "").catch(() => {});
      b.textContent = "скопировано";
    } else if (item && b.dataset.srt !== undefined) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const path = await save({ defaultPath: `${item.title}.srt`, filters: [{ name: "субтитры", extensions: ["srt"] }] });
      if (path) await invoke("video_srt", { id: item.id, path });
    } else if (item && b.dataset.del !== undefined) {
      await invoke("video_delete", { id: item.id });
      open = null;
      await render();
    } else if (item && b.dataset.speaker !== undefined) {
      // Щелчок по имени — переименовать прямо в строке.
      const input = Object.assign(document.createElement("input"), { className: "speaker-name", value: b.textContent ?? "" });
      b.replaceWith(input);
      input.focus(); input.select();
      const done = async () => { await invoke("video_rename_speaker", { id: item.id, speaker: Number(b.dataset.speaker), name: input.value }); await render(); };
      input.addEventListener("keydown", (k) => { if (k.key === "Enter") input.blur(); });
      input.addEventListener("blur", done, { once: true });
    }
  });
  // Правый клик по реплике — другой спикер или новый (как на маке, если распознало не того).
  root.addEventListener("contextmenu", async (e) => {
    const line = (e.target as HTMLElement).closest<HTMLElement>("[data-line]");
    const item = items.find((i) => i.id === open);
    if (!line || !item) return;
    e.preventDefault();
    const t = await invoke<Transcript | null>("video_get", { id: item.id });
    if (!t) return;
    const known = [...new Set(t.segments.map((s) => s.speaker).filter((s): s is number => s !== undefined))].sort();
    const next = known.length ? Math.max(...known) + 1 : 0;
    const mark = (speaker: number) => async () => { await invoke("video_mark", { id: item.id, index: Number(line.dataset.line), speaker }); await render(); };
    showMenu(e.clientX, e.clientY, [...known.map((s) => ({ label: t.names[s] ?? `спикер ${s + 1}`, run: mark(s) })), { label: "новый спикер", run: mark(next) }]);
  });
  sidebar.addEventListener("click", (e) => {
    if (!root.isConnected) return;
    const el = e.target as HTMLElement;
    const vf = el.closest<HTMLElement>("[data-vfilter]");
    if (vf) { filter = vf.dataset.vfilter as Filter; folderFilter = null; open = null; void render(); return; }
    const f = el.closest<HTMLElement>("[data-vfolder]");
    if (f) { folderFilter = f.dataset.vfolder!; open = null; void render(); return; }
    if (el.closest("[data-vnew]")) {
      // Имя вписывается прямо в строку (window.prompt в WebView есть не везде).
      const row = el.closest<HTMLElement>("[data-vnew]")!;
      const input = Object.assign(document.createElement("input"), { className: "new-folder", placeholder: "новая папка" });
      row.replaceChildren(input);
      input.focus();
      const item = items.find((i) => i.id === open);
      const done = async () => {
        const name = input.value.trim();
        if (name && item) await invoke("video_folder", { id: item.id, folder: name });
        if (name) folderFilter = name;
        await render();
      };
      input.addEventListener("keydown", (k) => { if (k.key === "Enter") input.blur(); if (k.key === "Escape") { input.value = ""; input.blur(); } });
      input.addEventListener("blur", () => void done(), { once: true });
      return;
    }
    const b = el.closest<HTMLElement>("[data-vid]");
    if (!b) return;
    open = b.dataset.vid || null;
    if (!open) folderFilter = null;
    void render();
  });
  // В папку — правым кликом по строке справа, как contextMenu у VideoRow.
  root.addEventListener("contextmenu", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-vrow]");
    const id = b?.dataset.vrow;
    if (!id || !root.isConnected) return;
    e.preventDefault();
    const folders = [...new Set(items.map((i) => i.folder).filter((f): f is string => !!f))].sort();
    showMenu(e.clientX, e.clientY, [
      ...folders.map((f) => ({ label: `в папку «${f}»`, run: async () => { await invoke("video_folder", { id, folder: f }); await render(); } })),
      { label: "вне папок", run: async () => { await invoke("video_folder", { id, folder: null }); await render(); } },
    ]);
  });
  const stop = await listen<Item>("video", () => { if (root.isConnected) void render(); else stop(); });
  await render();
}
