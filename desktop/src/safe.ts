// Раздел «пароли» — порт `SafeScreen.swift` (03.11.2026): сейф с паролями, кодами 2FA и секретами.
// Ключи — в памяти Rust, пока сейф открыт; на этом компьютере — в Диспетчере учётных данных, доступ по
// Windows Hello (на маке — Touch ID). Сам закрывается без дела, при блокировке экрана и после сна.

import { invoke } from "@tauri-apps/api/core";
import type { Prefs } from "./api";
import { icon, type Glyph } from "./icons";
import { showMenu } from "./menu";
import { os, hello } from "./platform";
import { blurIn } from "./motion";

type Kind = "login" | "totp" | "secret";
interface Item { id: string; kind: Kind; title: string; username: string; secret: string; url: string; note: string; modified: number }
interface State { exists: boolean; remembered: boolean; unlocked: boolean; hello: boolean }

const KINDS: [Kind, string, Glyph][] = [["login", "пароли", "key"], ["totp", "коды 2fa", "taskDone"], ["secret", "секреты", "spark"]];
const LOCKS = [30, 60, 300, 900, 3600, 0];
const lockTitle = (s: number) => s === 0 ? "никогда" : s < 60 ? `${s} сек` : s < 3600 ? `${s / 60} мин` : `${s / 3600} ч`;
// Шаги переноса — как на маке, но откуда выгружать на Windows: Edge, Chrome или «Пароли iCloud».
const STEPS = [
  os("открой Edge, Chrome или «Пароли iCloud» для Windows", "открой «Пароли» на маке, Chrome или Safari"),
  "выгрузи пароли в файл CSV: настройки → пароли → экспорт",
  "здесь нажми «выбрать файл» и укажи его",
  "удали этот файл и очисти корзину: в нём пароли открытым текстом",
];

export interface SafeHost {
  inTauri: boolean;
  prefs(): Prefs;
  setPrefs(p: Prefs): Promise<void>;
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const st = { kind: "login" as Kind, open: null as string | null, shown: false, adding: false, importing: false, own: false,
  error: null as string | null, items: [] as Item[], codes: {} as Record<string, string>, left: 30, lastUse: Date.now() };

/** Закрыть сейф снаружи (блокировка экрана, сон, простой). */
export async function lockSafe() {
  st.items = []; st.open = null;
  await invoke("safe_lock").catch(() => {});
}

let watching = false;
function watch(host: SafeHost, repaint: () => void) {
  if (watching || !host.inTauri) return;
  watching = true;
  let tick = Date.now();
  setInterval(async () => {
    const now = Date.now();
    const idle = host.prefs().safeLockSeconds ?? 300;
    // Шаг таймера пропал дольше чем на 10 с — компьютер спал.
    const slept = now - tick > 10_000;
    tick = now;
    if (!st.items.length && !(await invoke<State>("safe_state").catch(() => null))?.unlocked) return;
    if (slept || (idle > 0 && now - st.lastUse > idle * 1000) || (await invoke<boolean>("screen_locked"))) {
      await lockSafe(); repaint();
    }
  }, 2000);
}

export async function mountSafe(root: HTMLElement, sidebar: HTMLElement, host: SafeHost) {
  let state: State = { exists: false, remembered: false, unlocked: false, hello: false };
  const touch = () => (st.lastUse = Date.now());

  async function load() {
    state = await invoke<State>("safe_state");
    st.items = state.unlocked ? await invoke<Item[]>("safe_items") : [];
    if (state.unlocked) await codes();
    paint();
  }
  async function codes() {
    const [c, left] = await invoke<[Record<string, string>, number]>("safe_codes").catch(() => [{}, 30] as [Record<string, string>, number]);
    st.codes = c; st.left = left;
  }

  const coral = (act: string, text: string, off = false) => `<button class="coral-capsule" data-act="${act}"${off ? " disabled" : ""}>${text}</button>`;
  const quiet = (act: string, text: string) => `<button class="quiet" data-act="${act}">${text}</button>`;
  const input = (name: string, ph: string, value = "", type = "text") =>
    `<input class="safe-input" name="${name}" type="${type}" placeholder="${ph}" value="${esc(value)}" spellcheck="false" autocomplete="off">`;

  function lockScreen(): string {
    if (state.remembered && state.hello) return coral("hello", `открыть по ${hello}`);
    const creating = !state.exists;
    if (creating && !st.own) return `<div class="safe-actions">${coral("create", "создать сейф")}${quiet("own", "свой мастер-пароль")}</div>`;
    return `<form class="safe-form" data-form="${creating ? "create" : "unlock"}">
      ${input("p1", creating ? "придумай мастер-пароль" : "ключ сейфа или мастер-пароль", "", "password")}
      ${creating ? input("p2", "ещё раз", "", "password") + `<small>забудешь — сейф не откроет никто, даже мы. запиши пароль на бумаге</small><small class="weak" hidden>пароль слабый: если файл утечёт с github, его можно подобрать. лучше 12+ знаков, буквы, цифры и символы</small>` : ""}
      ${coral("submit", creating ? "создать сейф" : "открыть", true)}</form>`;
  }

  function addForm(): string {
    const k = st.kind;
    const fields = (k === "totp" ? input("secret", "секретный ключ или ссылка otpauth:// — из настроек 2fa на сайте") : "")
      + input("title", k === "totp" ? "сервис, например github" : "название")
      + (k !== "secret" ? input("username", k === "totp" ? "аккаунт" : "логин") : "")
      + (k === "login" ? `<div class="safe-line">${input("secret", "пароль")}${quiet("gen", "новый")}</div>${input("url", "сайт")}` : "")
      + (k === "secret" ? input("secret", "секрет") : "");
    return `<form class="safe-form safe-card" data-form="add"><b>${k === "login" ? "новый пароль" : k === "totp" ? "новый код 2fa" : "новый секрет"}</b>
      ${fields}<small class="safe-preview"></small>
      <div class="safe-actions">${coral("save-new", "сохранить", true)}${quiet("cancel", "отмена")}</div></form>`;
  }

  // Буква в коралловом круге 34 — Monogram на маке.
  const mono = (t: string) => `<span class="safe-mono">${esc(t.trim().slice(0, 1).toUpperCase())}</span>`;
  // Код 2FA карточкой (CodeCard): буква, сервис и аккаунт, кольцо секунд; крупный код по три цифры; клик — копия.
  const ring = (left: number) => { const c = 2 * Math.PI * 11.5; return `<svg class="safe-ring${left <= 5 ? " late" : ""}" viewBox="0 0 26 26"><circle cx="13" cy="13" r="11.5"/><circle class="arc" cx="13" cy="13" r="11.5" stroke-dasharray="${c}" stroke-dashoffset="${c * (1 - left / 30)}"/><text x="13" y="16.5">${left}</text></svg>`; };
  const split = (c: string | undefined) => c ? `${c.slice(0, 3)} ${c.slice(3)}` : "——— ———";
  const card = (i: Item) => `<div class="code-card${st.left <= 5 ? " late" : ""}" data-code="${i.id}"><div class="code-top">${mono(i.title)}<div><span>${esc(i.title)}</span><small>${esc(i.username)}</small></div>${ring(st.left)}</div>
    <b class="code-digits">${split(st.codes[i.id])}</b><button class="quiet-link code-edit" data-toggle="${i.id}">изменить</button></div>`;

  function row(i: Item): string {
    const open = st.open === i.id;
    const code = i.kind === "totp" ? `<b class="safe-code">${st.codes[i.id] ?? "—"}</b><small class="safe-left">${st.left}</small>` : "";
    const head = `<div class="safe-head" data-toggle="${i.id}">${mono(i.title)}<div><span>${esc(i.title)}</span><small>${esc(i.username || i.url)}</small></div>
      ${code}<button class="pill" data-copy="${i.id}">скопировать</button></div>`;
    if (!open) return `<div class="safe-row">${head}</div>`;
    const label = i.kind === "totp" ? "секрет или otpauth://" : "пароль";
    return `<form class="safe-row safe-form" data-form="edit" data-id="${i.id}">${head}
      ${input("title", "название", i.title)}
      ${i.kind !== "secret" ? input("username", i.kind === "totp" ? "аккаунт" : "логин", i.username) : ""}
      <div class="safe-line">${input("secret", label, i.secret, st.shown ? "text" : "password")}${quiet("show", st.shown ? "скрыть" : "показать")}
        ${i.kind === "login" ? quiet("gen", "новый") : ""}</div>
      ${i.kind === "login" ? input("url", "сайт", i.url) : ""}${input("note", "заметка", i.note)}
      <div class="safe-actions">${coral("save", "сохранить")}${quiet("delete", "удалить")}</div></form>`;
  }

  function paint() {
    const counts = (k: Kind) => st.items.filter((i) => i.kind === k).length;
    sidebar.innerHTML = state.unlocked ? KINDS.map(([k, t, g]) => `<button class="row${st.kind === k ? " selected" : ""}" style="padding-left:8px" data-kind="${k}">
        ${icon(g)}<span>${t}</span><small class="count">${counts(k)}</small></button>`).join("")
      + `<button class="row" style="padding-left:8px" data-side="key">${icon("key")}<span>ключ для другого компьютера</span></button>
         <button class="row" style="padding-left:8px" data-side="lock">${icon("settings")}<span>закрыть сейф</span></button>` : "";
    const title = state.unlocked ? KINDS.find(([k]) => k === st.kind)![1] : "пароли";
    const sub = state.unlocked ? "в зашифрованном файле .safe/safe.enc, на github — только шифром"
      : state.remembered && state.hello ? os("сейф открывается windows hello: лицо, палец или пин-код windows", "открой touch id или паролем mac")
      : state.exists ? os("на новом компьютере — ключ сейфа один раз, дальше windows hello", "введи мастер-пароль сейфа — дальше хватит touch id")
      : os("ключ придумает компьютер и спрячет в диспетчер учётных данных — открывать будешь windows hello", "ключ придумает компьютер и спрячет в связку ключей — открывать будешь touch id");
    let body = "";
    if (!state.unlocked) body = lockScreen();
    else {
      const p = host.prefs();
      const list = st.items.filter((i) => i.kind === st.kind);
      body = `<div class="safe-actions">${coral("add", "добавить")}<button class="outline-pill" data-act="import">${st.importing ? "скрыть перенос" : os("перенести из браузера", "перенести из «Паролей»")}</button></div>`
        + (st.adding ? addForm() : "")
        + (st.importing || (!st.items.length && !st.adding) ? `<div class="safe-guide">${STEPS.map((s, i) => `<p><b>${i + 1}</b>${s}</p>`).join("")}
            <div class="safe-actions"><button class="pill" data-act="pick">выбрать файл</button></div></div>` : "")
        + (!st.adding && st.items.length && !list.length ? `<small class="safe-empty">здесь пока пусто — нажми «добавить»</small>` : "")
        + (st.kind === "totp" ? (list.some((i) => i.id === st.open) ? row(list.find((i) => i.id === st.open)!) : "") + `<div class="code-cards">${list.map(card).join("")}</div>` : list.map(row).join(""))
        + `<div class="setting"><div><b>закрывать сам</b><small>${(p.safeLockSeconds ?? 300) === 0 ? "никогда — опасно: открытый сейф увидит любой, кто сел за компьютер"
            : `без дела ${lockTitle(p.safeLockSeconds ?? 300)}, а ещё при сне и блокировке экрана`}</small></div>
            <button class="pill" data-act="lock-after">${lockTitle(p.safeLockSeconds ?? 300)}</button></div>`;
    }
    root.innerHTML = `<div class="settings safe"><h1>${title}</h1><p class="settings-sub">${sub}</p>
      ${st.error ? `<p class="sync-error">${esc(st.error)}</p>` : ""}<div class="settings-rows">${body}</div></div>`;
    root.querySelector<HTMLInputElement>(".safe-form input")?.focus();
  }

  const formItem = (f: HTMLFormElement, base: Item): Item => {
    const v = (n: string) => (f.elements.namedItem(n) as HTMLInputElement | null)?.value ?? (base as unknown as Record<string, string>)[n];
    return { ...base, title: v("title").trim(), username: v("username"), secret: v("secret"), url: v("url"), note: v("note") };
  };
  const blank = (): Item => ({ id: "", kind: st.kind, title: "", username: "", secret: "", url: "", note: "", modified: 0 });

  async function act(a: string, el: HTMLElement) {
    touch();
    st.error = null;
    const form = el.closest<HTMLFormElement>("form");
    try {
      if (a === "hello") { if (!(await invoke<boolean>("safe_unlock_hello"))) return; }
      else if (a === "create") await invoke("safe_create", { password: null });
      else if (a === "own") st.own = true;
      else if (a === "submit" && form) {
        const p1 = (form.elements.namedItem("p1") as HTMLInputElement).value;
        if (form.dataset.form === "create") await invoke("safe_create", { password: p1 });
        else await invoke("safe_unlock", { text: p1 });
      } else if (a === "add") { st.adding = !st.adding; st.importing = false; }
      else if (a === "import") { st.importing = !st.importing; st.adding = false; }
      else if (a === "cancel") st.adding = false;
      else if (a === "gen" && form) { (form.elements.namedItem("secret") as HTMLInputElement).value = await invoke<string>("safe_generate"); return; }
      else if (a === "show") st.shown = !st.shown;
      else if (a === "save-new" && form) { await invoke("safe_put", { item: formItem(form, blank()) }); st.adding = false; }
      else if (a === "save" && form) { await invoke("safe_put", { item: formItem(form, st.items.find((i) => i.id === form.dataset.id)!) }); st.open = null; }
      else if (a === "delete" && form) { await invoke("safe_delete", { id: form.dataset.id }); st.open = null; }
      else if (a === "pick") {
        const { open } = await import("@tauri-apps/plugin-dialog");
        const path = await open({ filters: [{ name: "CSV", extensions: ["csv"] }] });
        if (!path) return;
        const n = await invoke<number>("safe_import", { path });
        st.importing = false;
        st.error = `перенесено: ${n}. удали CSV — в нём пароли открытым текстом`;
      } else if (a === "lock-after") {
        const r = el.getBoundingClientRect();
        showMenu(r.left, r.bottom + 4, LOCKS.map((s) => ({ label: lockTitle(s), run: async () => { await host.setPrefs({ ...host.prefs(), safeLockSeconds: s }); paint(); } })));
        return;
      }
    } catch (e) { st.error = String(e); }
    await load();
    if (st.error && a !== "pick") paint();
  }

  root.onclick = async (e) => {
    const t = e.target as HTMLElement;
    const copy = t.closest<HTMLElement>("[data-copy]");
    if (copy) { touch(); await invoke("safe_copy", { what: "item", id: copy.dataset.copy }); copy.textContent = "скопировано"; return; }
    const cardEl = t.closest<HTMLElement>("[data-code]");
    if (cardEl && !t.closest("[data-toggle]")) {
      touch(); await invoke("safe_copy", { what: "item", id: cardEl.dataset.code });
      const d = cardEl.querySelector<HTMLElement>(".code-digits")!; d.textContent = "скопировано"; cardEl.dataset.copied = "1";
      setTimeout(() => { delete cardEl.dataset.copied; d.textContent = split(st.codes[cardEl.dataset.code!]); }, 1200);
      return;
    }
    const a = t.closest<HTMLElement>("[data-act]");
    if (a) { e.preventDefault(); return void act(a.dataset.act!, a); }
    const toggle = t.closest<HTMLElement>("[data-toggle]");
    if (toggle && !t.closest("input")) { touch(); st.open = st.open === toggle.dataset.toggle ? null : toggle.dataset.toggle!; st.shown = false; paint(); }
  };
  root.onsubmit = (e) => { e.preventDefault(); const b = (e.target as HTMLElement).querySelector<HTMLElement>(".coral-capsule"); if (b && !b.hasAttribute("disabled")) void act(b.dataset.act!, b); };
  root.oninput = async (e) => {
    touch();
    const f = (e.target as HTMLElement).closest<HTMLFormElement>("form");
    if (!f) return;
    const val = (n: string) => (f.elements.namedItem(n) as HTMLInputElement | null)?.value ?? "";
    const submit = f.querySelector<HTMLButtonElement>(".coral-capsule")!;
    if (f.dataset.form === "create") {
      submit.disabled = !val("p1") || val("p1") !== val("p2");
      f.querySelector<HTMLElement>(".weak")!.hidden = !val("p1") || !(await invoke<boolean>("safe_weak", { password: val("p1") }));
    } else if (f.dataset.form === "unlock") submit.disabled = !val("p1");
    else if (f.dataset.form === "add") {
      // Ссылка otpauth://totp/GitHub:makar?secret=…&issuer=GitHub — название и аккаунт из неё.
      const s = val("secret");
      if (st.kind === "totp" && s.startsWith("otpauth://")) {
        try {
          const u = new URL(s);
          const label = decodeURIComponent(u.pathname.replace(/^\/+/, "")).split(":");
          const t = f.elements.namedItem("title") as HTMLInputElement, n = f.elements.namedItem("username") as HTMLInputElement;
          if (!t.value) t.value = u.searchParams.get("issuer") ?? (label.length === 2 ? label[0] : "");
          if (!n.value) n.value = label[label.length - 1] ?? "";
        } catch { /* не ссылка */ }
      }
      const code = st.kind === "totp" ? await invoke<string | null>("safe_totp", { secret: s }) : "";
      f.querySelector<HTMLElement>(".safe-preview")!.textContent = st.kind === "totp" && code ? `код сейчас: ${code}` : "";
      submit.disabled = !val("title").trim() || !s || (st.kind === "totp" && !code);
    }
  };
  sidebar.onclick = async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-kind],[data-side]");
    if (!b || !root.isConnected) return;
    touch();
    if (b.dataset.kind) { st.kind = b.dataset.kind as Kind; st.open = null; st.adding = false; paint(); blurIn(root); }
    else if (b.dataset.side === "key") { await invoke("safe_copy", { what: "key" }); b.querySelector("span")!.textContent = "ключ скопирован на минуту"; }
    else { await lockSafe(); await load(); }
  };
  // Коды 2FA меняются каждые 30 с — обновляем только цифры, не трогая открытую форму.
  const timer = setInterval(async () => {
    if (!root.isConnected) return clearInterval(timer);
    if (!state.unlocked || !st.items.some((i) => i.kind === "totp")) return;
    await codes();
    root.querySelectorAll<HTMLElement>(".safe-row").forEach((r) => {
      const id = r.querySelector<HTMLElement>("[data-toggle]")?.dataset.toggle;
      const c = r.querySelector(".safe-code"), l = r.querySelector(".safe-left");
      if (id && c) c.textContent = st.codes[id] ?? "—";
      if (l) l.textContent = String(st.left);
    });
    root.querySelectorAll<HTMLElement>(".code-card").forEach((c) => {
      c.classList.toggle("late", st.left <= 5);
      // Кольцо не пересоздаём — меняем дугу, и она плавно убывает (.linear(duration: 1) на маке).
      const r = c.querySelector<SVGElement>(".safe-ring")!, len = 2 * Math.PI * 11.5;
      r.classList.toggle("late", st.left <= 5);
      r.querySelector(".arc")!.setAttribute("stroke-dashoffset", String(len * (1 - st.left / 30)));
      r.querySelector("text")!.textContent = String(st.left);
      if (!c.dataset.copied) c.querySelector(".code-digits")!.textContent = split(st.codes[c.dataset.code!]);
    });
  }, 1000);
  watch(host, () => { if (root.isConnected) void load(); });
  await load();
  if (state.remembered && state.hello && !state.unlocked) void act("hello", root);
}
