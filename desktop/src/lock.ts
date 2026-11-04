// Вход по пинкоду и Windows Hello — порт `UnlockStep.swift` и `PinEntrySheet.swift`.
// Экран входа: «введи пинкод», подпись, четыре ячейки, «забыл пинкод»; четвёртая цифра проверяется сразу.
// Неверный — ячейки качаются и пустеют; после 5 ошибок — пауза с обратным отсчётом.
// Карточка «придумай пинкод → повтори → не совпало» — в настройках → вход.

import { invoke } from "@tauri-apps/api/core";
import { os, hello } from "./platform";

const LENGTH = 4;
const nowS = () => Math.floor(Date.now() / 1000);

export interface LockState { has_pin: boolean; needs_unlock: boolean; hello: boolean; wait: number | null }

export const lockApi = {
  state: () => invoke<LockState>("lock_state", { now: nowS() }),
  seen: () => invoke<void>("lock_seen", { now: nowS() }),
  neededOnReturn: (since: number) => invoke<boolean>("lock_needed_on_return", { inactiveSince: since, now: nowS() }),
  set: (pin: string) => invoke<void>("pin_set", { pin }),
  clear: () => invoke<void>("pin_clear"),
  check: (pin: string) => invoke<{ ok: boolean; wait: number | null }>("pin_check", { pin, now: nowS() }),
  hello: () => invoke<boolean>("hello_verify"),
};

const cells = (filled: number, current: number | null, error: boolean) =>
  `<div class="pin-cells${error ? " error" : ""}">${Array.from({ length: LENGTH }, (_, i) =>
    `<i class="${i < filled ? "filled" : ""}${i === current ? " current" : ""}"></i>`).join("")}</div>`;

const waitText = (s: number) => (s < 60 ? `${s} с` : `${Math.ceil(s / 60)} мин`);

/** Экран входа поверх приложения. Возвращает промис, который разрешится после входа. */
export function showUnlock(state: LockState): Promise<void> {
  return new Promise((resolve) => {
    const el = document.createElement("div");
    el.className = "unlock";
    document.body.append(el);
    document.getElementById("app")!.classList.add("locked");
    let digits = "";
    let wrong = false;
    let wait = state.wait;
    let busy = false;

    const render = () => {
      const subtitle = wait ? `слишком много попыток, подожди ${waitText(wait)}`
        : wrong ? "неверный пинкод"
        : state.hello ? os("или войдите через windows hello", "или войдите через touch id") : "чтобы открыть заметки";
      el.innerHTML = `<div class="unlock-sheet">
        <h1>введи пинкод</h1>
        <p class="${wrong || wait ? "error" : ""}">${subtitle}</p>
        <div class="${wait ? "waiting" : ""}">${cells(digits.length, wait ? null : digits.length, wrong && !wait)}</div>
        ${state.hello ? `<button class="quiet" data-hello>${hello}</button>` : ""}
        <button class="quiet forgot" data-forgot>забыл пинкод</button>
      </div>`;
    };

    const finish = async () => {
      document.removeEventListener("keydown", onKey, true);
      clearInterval(timer);
      await lockApi.seen();
      el.classList.add("leaving");
      document.getElementById("app")!.classList.remove("locked");
      setTimeout(() => el.remove(), 600);
      resolve();
    };

    const onKey = async (e: KeyboardEvent) => {
      e.stopPropagation();
      if (busy || wait) return;
      if (e.key === "Backspace") { digits = digits.slice(0, -1); render(); return; }
      if (!/^[0-9]$/.test(e.key) || digits.length >= LENGTH) return;
      digits += e.key;
      wrong = false;
      render();
      if (digits.length < LENGTH) return;
      busy = true;
      const r = await lockApi.check(digits).catch(() => ({ ok: false, wait: null }));
      busy = false;
      if (r.ok) { await finish(); return; }
      digits = "";
      wrong = true;
      wait = r.wait;
      render();
      el.querySelector(".pin-cells")?.classList.add("shake");
    };

    const askHello = async () => {
      if (!state.hello || busy) return;
      busy = true;
      const ok = await lockApi.hello().catch(() => false);
      busy = false;
      if (ok) await finish();
    };

    // «Забыл пинкод» — как на маке (пароль от Mac или Touch ID): Windows Hello, а он сам предложит
    // PIN или пароль от Windows; подтвердили — придумать новый пинкод.
    const forgot = async () => {
      if (busy) return;
      busy = true;
      const ok = await lockApi.hello().catch(() => false);
      busy = false;
      if (!ok) {
        wrong = false;
        el.querySelector("p")!.textContent = os("не подтвердилось: нужен вход в windows (параметры → учётные записи → варианты входа)", "не подтвердилось: включи touch id в системных настройках");
        return;
      }
      await lockApi.clear();
      document.removeEventListener("keydown", onKey, true);
      await choosePin();
      await finish();
    };
    el.addEventListener("click", (e) => {
      const t = e.target as HTMLElement;
      if (t.closest("[data-hello]")) void askHello();
      if (t.closest("[data-forgot]")) void forgot();
    });
    document.addEventListener("keydown", onKey, true);
    // Обратный отсчёт паузы идёт сам.
    const timer = setInterval(() => { if (wait) { wait = wait > 1 ? wait - 1 : null; render(); } }, 1000);
    render();
    requestAnimationFrame(() => el.classList.add("shown"));
    void askHello();
  });
}

/** Карточка настройки пинкода: придумай → повтори → не совпало. true — сохранён. */
export function choosePin(): Promise<boolean> {
  return new Promise((resolve) => {
    const el = document.createElement("div");
    el.className = "pin-overlay";
    document.body.append(el);
    let stage: "create" | "confirm" | "mismatch" | "done" = "create";
    let first = "";
    let digits = "";

    const render = () => {
      const title = stage === "create" ? "придумай пинкод" : stage === "mismatch" ? "не совпало" : "повтори пинкод";
      const sub = stage === "create" ? "4 цифры при входе" : stage === "mismatch" ? "придумай заново" : "ещё раз, чтобы не ошибиться";
      el.innerHTML = `<div class="pin-card">
        <button class="quiet" data-cancel>отмена</button>
        <h2>${title}</h2><p class="${stage === "mismatch" ? "error" : ""}">${sub}</p>
        ${cells(stage === "done" ? LENGTH : digits.length, stage === "create" || stage === "confirm" ? digits.length : null, stage === "mismatch")}
      </div>`;
    };
    const close = (saved: boolean) => {
      document.removeEventListener("keydown", onKey, true);
      el.classList.remove("shown");
      setTimeout(() => el.remove(), 500);
      resolve(saved);
    };
    const onKey = async (e: KeyboardEvent) => {
      e.stopPropagation();
      if (e.key === "Escape") { close(false); return; }
      if (stage === "done") return;
      if (e.key === "Backspace") { digits = digits.slice(0, -1); render(); return; }
      if (!/^[0-9]$/.test(e.key)) return;
      if (stage === "mismatch") stage = "create";
      digits += e.key;
      if (digits.length === LENGTH) {
        if (stage === "create") { first = digits; digits = ""; stage = "confirm"; }
        else if (digits === first) {
          stage = "done";
          render();
          await lockApi.set(first);
          setTimeout(() => close(true), 350);
          return;
        } else { first = ""; digits = ""; stage = "mismatch"; render(); el.querySelector(".pin-cells")?.classList.add("shake"); return; }
      }
      render();
    };
    el.addEventListener("click", (e) => { if ((e.target as HTMLElement).closest("[data-cancel]")) close(false); });
    document.addEventListener("keydown", onKey, true);
    render();
    requestAnimationFrame(() => el.classList.add("shown"));
  });
}
