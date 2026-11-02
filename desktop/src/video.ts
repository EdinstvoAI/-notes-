// Экран «видео» — порт `VideoScreen.swift`: зона «перетащи файл», список расшифровок слева, расшифровка
// с временем и спикерами справа; «сохранить как заметку», копировать, .srt, удалить.
// Распознаёт тот же GigaAM, что и надиктовка, на этом компьютере.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { icon } from "./icons";

interface Item { id: string; title: string; source: string; added: string; duration: number; words: number | null; error?: string | null }
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

  const renderSidebar = () => {
    sidebar.innerHTML = `<button class="row${open === null ? " selected" : ""}" style="padding-left:8px" data-vid="">${icon("video")}<span>все расшифровки</span><small class="count">${items.length}</small></button>`
      + items.map((i) => `<button class="row${open === i.id ? " selected" : ""}" style="padding-left:26px" data-vid="${i.id}">
          <span>${esc(i.title)}</span><small class="count">${i.error ? "!" : i.words === null ? "…" : ""}</small></button>`).join("");
  };

  const dropZone = () => `<div class="video">
    <h1>видео</h1><p class="video-sub">расшифровка на этом компьютере · ${items.length} ${items.length === 1 ? "файл" : "файлов"}</p>
    <button class="drop-zone" data-pick>
      <span class="drop-slats"></span>
      <b>перетащите видео или аудио</b><small>${TYPES.join(", ")}</small>
    </button></div>`;

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
      return `<div class="line">${who}<time>${clock(s.start)}</time><p>${esc(s.text)}</p></div>`;
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
    renderSidebar();
    const item = items.find((i) => i.id === open);
    host.onCrumb(item ? `видео / ${item.title}` : "видео");
    if (item) await renderItem(item); else root.innerHTML = dropZone();
  }

  root.addEventListener("click", async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    const item = items.find((i) => i.id === open);
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
  sidebar.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-vid]");
    if (!b || !root.isConnected) return;
    open = b.dataset.vid || null;
    void render();
  });
  const stop = await listen<Item>("video", () => { if (root.isConnected) void render(); else stop(); });
  await render();
}
