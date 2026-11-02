// Сфера надиктовки — окно поверх всех (как NSPanel на маке). Рисует шейдер `Orb.metal`, перенесённый на
// WebGL (gl.ts): слушает — полосы света от голоса, думает — кольца, готово — зелёная заливка и испарение.

import { listen } from "@tauri-apps/api/event";
import { ORB, shader } from "./gl";

type State = "listening" | "recognizing" | "copied" | "error" | "idle" | "level";
const canvas = document.getElementById("orb") as HTMLCanvasElement;
const draw = shader(canvas, ORB);
let state: State = "idle", level = 0, smooth = 0, think = 0, doneAt = 0;
const start = performance.now();
const dark = () => matchMedia("(prefers-color-scheme: dark)").matches ? 1 : 0;

function frame(now: number) {
  const t = (now - start) / 1000;
  // Голос сглаживается каждый кадр (OrbClock на маке); «думает» нарастает плавно.
  smooth += (level - smooth) * 0.18;
  think += ((state === "recognizing" ? 1 : 0) - think) * 0.08;
  const done = state === "copied" ? Math.min(1, (now - doneAt) / 1100) : 0;
  // Четыре полосы частот: пока одна громкость — низ сильнее, верх тише (у речи так).
  const bands = [smooth, smooth * 0.85, smooth * 0.65, smooth * 0.45].map((b, i) => b * (0.85 + 0.15 * Math.sin(t * 5 + i)));
  if (state !== "idle") draw?.({ t, dark: dark(), canvas: 1.9, think, done, bands });
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

void listen<{ state: State; level?: number }>("dictation", (e) => {
  if (e.payload.state === "level") { level = e.payload.level ?? 0; return; }
  if (e.payload.state === "copied") doneAt = performance.now();
  state = e.payload.state;
});
