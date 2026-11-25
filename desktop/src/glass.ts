// Единый материал для всех окон: движок и его CSS-резерв — из Edinstvo Glass.
// Поверхности живут отдельно от содержимого: innerHTML может заменять меню и разделы целиком.
import { attachGlass, type GlassOptions } from "@edinstvo/glass";

const selector = [
  ".topbar .group", ".sheet", ".crumb", ".search-panel", ".menu", ".cal-draft",
  ".unlock-sheet", ".pin-card", ".toast", ".board-card", ".hover-hint", ".tray",
  ".cal-modes", ".new-task", ".device-code.big", ".outline-pill",
  ".pill:not(.primary-pill):not(.danger)",
].join(",");
type Surface = ReturnType<typeof attachGlass>;
const surfaces = new Map<HTMLElement, { surface: Surface; canvas: HTMLCanvasElement }>();
const still = { follow: 0, press: 0, jelly: 0, glow: 0, morph: 0, merge: 0 };
let pending = 0;
let started = false;
let motionFrame = 0;

// Фон листов (основной и первого входа/пинкода) — цвет страницы. Сам («auto») лист берёт элемент под своим
// центром, а там чужое: строка настроек рядом с листом — белое пятно (issue #17).
const pageColor = document.createElement("canvas");
pageColor.width = pageColor.height = 1;
function paintPageColor() {
  const context = pageColor.getContext("2d")!;
  context.clearRect(0, 0, 1, 1);
  context.fillStyle = getComputedStyle(document.body).backgroundColor;
  context.fillRect(0, 0, 1, 1);
  return pageColor;
}

function options(host: HTMLElement): GlassOptions {
  // Шапка преломляет настоящие планки, лист — цвет страницы; всплывающие поверхности находят свой фон сами.
  const slats = host.closest(".topbar") ? document.querySelector<HTMLCanvasElement>("#app > .slats") : null;
  return {
    source: slats ?? (host.matches(".sheet, .unlock-sheet") ? paintPageColor() : "auto"), live: false, motion: still,
    material: { frost: host.matches(".topbar .group, .crumb") ? 0.45 : 0.7,
      refraction: 0.3, depth: 6, tint: host.matches(".pin-card") ? "#fff" : "auto",
      transparency: host.matches(".pin-card") ? 0.1 : 0.85 },
  };
}

// Пока идёт первый вход, приложение под ним закрыто непрозрачным слоем, но библиотека подмешивает стекло
// нижних поверхностей в верхние — ободы его листа светились полосами по краям листа шагов (issue #9).
const wanted = (host: HTMLElement) => host.matches(selector) && !(host.closest("#app") && document.querySelector(".unlock.onboarding"));

function sync() {
  pending = 0;
  for (const [host, entry] of surfaces) {
    if (!host.isConnected || !host.contains(entry.canvas) || !wanted(host)) {
      entry.surface.destroy();
      host.classList.remove("notes-glass", "glass");
      delete host.dataset.engine;
      surfaces.delete(host);
    }
  }
  for (const host of document.querySelectorAll<HTMLElement>(selector)) {
    if (surfaces.has(host) || !wanted(host)) continue;
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

// CSS двигает контейнер, а библиотека не видит его transform через ResizeObserver.
// Пересчитываем линзы на кадрах только пока движется их контейнер.
function followMotion() {
  motionFrame = 0;
  const moving = document.getAnimations().some(animation => {
    const effect = animation.effect;
    if (!(effect instanceof KeyframeEffect) || !(effect.target instanceof Element)) return false;
    const geometry = ["transform", "translate", "scale", "rotate", "width", "height", "left", "top"];
    if (!effect.getKeyframes().some(frame => geometry.some(key => key in frame))) return false;
    const target = effect.target;
    return [...surfaces.keys()].some(host => target === host || target.contains(host));
  });
  if (!moving) return;
  for (const { surface } of surfaces.values()) surface.invalidate();
  motionFrame = requestAnimationFrame(followMotion);
}
function startMotion() { motionFrame ||= requestAnimationFrame(followMotion); }

export function startGlass() {
  if (started) return;
  started = true;
  new MutationObserver((records) => {
    // Сам движок меняет style/data-engine: их не наблюдаем, чтобы не запускать цикл кадров.
    if (records.some((r) => r.type === "childList" || r.attributeName === "class")) schedule();
  }).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["class"] });
  new MutationObserver(() => refreshGlass()).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", refreshGlass);
  document.body.addEventListener("transitionrun", startMotion);
  document.body.addEventListener("animationstart", startMotion);
  sync();
}

/** Планки рисуются по событию, поэтому постоянный цикл кадров библиотеке не нужен. */
export function refreshGlass() {
  for (const [host, { surface }] of surfaces) surface.set(options(host));
  schedule();
}
