// Единый материал для всех окон: движок и его CSS-резерв — из Edinstvo Glass.
// Поверхности живут отдельно от содержимого: innerHTML может заменять меню и разделы целиком.
import { attachGlass, type GlassOptions } from "@edinstvo/glass";

const selector = [
  ".topbar .group", ".sheet", ".crumb", ".search-panel", ".menu", ".cal-draft",
  ".unlock-sheet", ".pin-card", ".toast", ".board-card", ".hover-hint", ".tray",
].join(",");
type Surface = ReturnType<typeof attachGlass>;
const surfaces = new Map<HTMLElement, { surface: Surface; canvas: HTMLCanvasElement }>();
const still = { follow: 0, press: 0, jelly: 0, glow: 0, morph: 0, merge: 0 };
let pending = 0;
let started = false;

function options(host: HTMLElement): GlassOptions {
  // Шапка преломляет настоящие планки; всплывающие поверхности находят свой фон сами.
  const slats = host.closest(".topbar") ? document.querySelector<HTMLCanvasElement>("#app > .slats") : null;
  return {
    source: slats ?? "auto", live: false, motion: still,
    material: { frost: host.matches(".topbar .group, .crumb") ? 0.45 : 0.7,
      refraction: 0.3, depth: 6, tint: host.matches(".pin-card") ? "#fff" : "auto",
      transparency: host.matches(".pin-card") ? 0.1 : 0.85 },
  };
}

function sync() {
  pending = 0;
  for (const [host, entry] of surfaces) {
    if (!host.isConnected || !host.contains(entry.canvas) || !host.matches(selector)) {
      entry.surface.destroy();
      host.classList.remove("notes-glass", "glass");
      delete host.dataset.engine;
      surfaces.delete(host);
    }
  }
  for (const host of document.querySelectorAll<HTMLElement>(selector)) {
    if (surfaces.has(host)) continue;
    host.classList.add("notes-glass", "glass");
    const surface = attachGlass(host, options(host));
    const canvas = host.querySelector<HTMLCanvasElement>(":scope > canvas")!;
    canvas.classList.add("notes-glass-canvas");
    // Отрицательный слой сохраняет текстовые узлы и абсолютные элементы поверх линзы.
    canvas.style.zIndex = "-1";
    surfaces.set(host, { surface, canvas });
  }
}

function schedule() { pending ||= requestAnimationFrame(sync); }

export function startGlass() {
  if (started) return;
  started = true;
  new MutationObserver((records) => {
    // Сам движок меняет style/data-engine: их не наблюдаем, чтобы не запускать цикл кадров.
    if (records.some((r) => r.type === "childList" || r.attributeName === "class")) schedule();
  }).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["class"] });
  new MutationObserver(() => refreshGlass()).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", refreshGlass);
  sync();
}

/** Планки рисуются по событию, поэтому постоянный цикл кадров библиотеке не нужен. */
export function refreshGlass() {
  for (const [host, { surface }] of surfaces) surface.set(options(host));
  schedule();
}
