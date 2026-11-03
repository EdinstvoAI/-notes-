// Сфера надиктовки — окно поверх всех (как NSPanel на маке). Рисует шейдер `Orb.metal`, перенесённый на
// WebGL (gl.ts): слушает — полосы света от голоса, думает — кольца, готово — зелёная заливка и испарение.

import { listen } from "@tauri-apps/api/event";
import { ORB, shader } from "./gl";

type State = "listening" | "recognizing" | "copied" | "error" | "idle" | "level";
const canvas = document.getElementById("orb") as HTMLCanvasElement;
const draw = shader(canvas, ORB);
let state: State = "idle", level = 0, smooth = 0, think = 0, doneAt = 0;
let target = [0, 0, 0, 0], shown = [0, 0, 0, 0];
const start = performance.now();
const dark = () => matchMedia("(prefers-color-scheme: dark)").matches ? 1 : 0;

function frame(now: number) {
  const t = (now - start) / 1000;
  // Голос сглаживается каждый кадр (OrbClock на маке); «думает» нарастает плавно.
  smooth += (level - smooth) * 0.18;
  think += ((state === "recognizing" ? 1 : 0) - think) * 0.08;
  const done = state === "copied" ? Math.min(1, (now - doneAt) / 1100) : 0;
  // Четыре полосы частот голоса, сглаженные каждый кадр.
  shown = shown.map((v, i) => v + (target[i] - v) * 0.18);
  const bands = shown;
  if (state !== "idle") draw?.({ t, dark: dark(), canvas: 1.9, think, done, bands });
  // В покое кадры не крутим: окно сферы живёт всё время, и пустой цикл грел процессор.
  if (state === "idle") running = false; else requestAnimationFrame(frame);
}
let running = false;
const kick = () => { if (!running) { running = true; requestAnimationFrame(frame); } };

void listen<{ state: State; level?: number; bands?: number[] }>("dictation", (e) => {
  if (e.payload.state === "level") { level = e.payload.level ?? 0; target = e.payload.bands ?? [level, level, level, level]; return; }
  if (e.payload.state === "copied") doneAt = performance.now();
  state = e.payload.state;
  kick();
});
