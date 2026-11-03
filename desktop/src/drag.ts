// Перетаскивание в боковой панели, как на маке (`NotesModel.drop`): заметку или папку тянут на папку —
// переносим; мимо папок, на пустое место панели — в корень. Файлы из Проводника или Finder копируются внутрь.
//
// Своё перетаскивание — указателем, а не HTML5 drag-and-drop: на Windows WebView2 при включённом
// приёме файлов из Проводника (а он нужен) HTML5-перетаскивание внутри страницы не работает.

export interface DragHost {
  /** Перенести `path` в папку `folder` ("" — корень). */
  move(path: string, folder: string): Promise<void>;
  /** Скопировать внутрь файлы снаружи. */
  copyIn(paths: string[], folder: string): Promise<void>;
}

const THRESHOLD = 5;

function folderAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  const row = el?.closest<HTMLElement>("[data-folder]");
  if (row) return row.dataset.folder ?? "";
  if (el?.closest(".sidebar")) return "";
  return null;
}

function mark(folder: string | null) {
  document.querySelectorAll(".drop-target").forEach((e) => e.classList.remove("drop-target"));
  if (folder === null) return;
  const row = folder === "" ? document.querySelector(".sidebar") : document.querySelector(`.sidebar [data-folder="${CSS.escape(folder)}"]`);
  row?.classList.add("drop-target");
}

export function enableDrag(root: HTMLElement, host: DragHost) {
  root.addEventListener("pointerdown", (down) => {
    if (down.button !== 0) return;
    const row = (down.target as HTMLElement).closest<HTMLElement>(".sidebar .row[data-note], .sidebar .row[data-folder]");
    if (!row || row.dataset.pinned !== undefined) return;
    const path = row.dataset.note ?? row.dataset.folder!;
    let ghost: HTMLElement | null = null;
    let target: string | null = null;

    const move = (e: PointerEvent) => {
      if (!ghost) {
        if (Math.hypot(e.clientX - down.clientX, e.clientY - down.clientY) < THRESHOLD) return;
        ghost = row.cloneNode(true) as HTMLElement;
        ghost.className = "row drag-ghost";
        ghost.style.width = `${row.offsetWidth}px`;
        document.body.append(ghost);
        row.classList.add("dragging");
      }
      ghost.style.transform = `translate(${e.clientX + 8}px, ${e.clientY - 13}px)`;
      target = folderAt(e.clientX, e.clientY);
      // Папку в саму себя и в свою подпапку не кладём — и не подсвечиваем.
      if (target !== null && (target === path || target.startsWith(path + "/"))) target = null;
      mark(target);
    };
    const up = async () => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
      mark(null);
      row.classList.remove("dragging");
      if (!ghost) return;
      ghost.remove();
      // Клик после перетаскивания не должен открывать строку.
      // Браузер шлёт его не всегда — без таймера блокировка съела бы следующий настоящий клик.
      const swallow = (e: MouseEvent) => { e.stopPropagation(); e.preventDefault(); };
      addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => removeEventListener("click", swallow, { capture: true }), 0);
      if (target !== null) await host.move(path, target);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
  });
}

/** Файлы из Проводника или Finder: событие окна Tauri с путями и точкой в физических пикселях. */
export async function enableFileDrop(host: DragHost) {
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  await getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    // Зона видео подсвечивается кораллом и зовёт «отпусти» — как targeted у DropZone на маке.
    const zone = document.querySelector<HTMLElement>(".drop-zone");
    if (zone) {
      const on = p.type === "enter" || p.type === "over";
      zone.classList.toggle("targeted", on);
      const label = zone.querySelector("b");
      if (label) label.textContent = on ? "отпусти, чтобы расшифровать" : "перетащи видео или аудио";
    }
    if (p.type === "leave") { mark(null); return; }
    const ratio = devicePixelRatio || 1;
    const folder = folderAt(p.position.x / ratio, p.position.y / ratio);
    if (p.type === "over") mark(folder);
    else if (p.type === "drop") {
      mark(null);
      if (folder !== null) void host.copyIn(p.paths, folder);
    } else if (p.type === "enter") mark(folder);
  });
}
