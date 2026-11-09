// Настройки — порт `SettingsScreen.swift`: страницы в боковой панели, заголовок и серая подпись,
// стеклянные строки 640 с кнопкой справа. Здесь — то, что уже работает на Windows; остальное
// (git и GitHub, надиктовка, пинкод) — отдельной строкой «переезжает с мака», без выдуманного вида.

import type { Api, Prefs } from "./api";
import { icon, type Glyph } from "./icons";
import { choosePin, lockApi, type LockState } from "./lock";
import { os, hello } from "./platform";
import { blurIn } from "./motion";
import { showMenu } from "./menu";

type Page = "general" | "sync" | "search" | "templates" | "calendar" | "dictation" | "lock";
// Порядок, подписи и значки — как NotesModel.SettingsPage и subtitle(_:) на маке.
const PAGES: [Page, string, string, Glyph][] = [
  ["general", "общие", "как выглядит и запускается", "settings"],
  ["sync", "синхронизация", "хранилище, история и github", "git"],
  ["dictation", "надиктовка", "говоришь — текст появляется там, где курсор", "microphone"],
  ["search", "поиск", "заметки, задачи, события, видео", "search"],
  ["lock", "вход", os("пинкод и windows hello", "пинкод и touch id"), "check"],
  ["calendar", "календарь", "задачи со сроком попадают в календарь", "calendar"],
  ["templates", "шаблоны", "шаблоны и заметка на каждый день", "file"],
];

export interface SettingsHost {
  backend: Api;
  onCrumb(text: string): void;
  /** Применить тему и размер текста сразу. */
  apply(prefs: Prefs): void;
  pickVault(): Promise<void>;
  /** Создать папку в корне, только если её ещё нет (иначе вышла бы «шаблоны 2»). */
  ensureFolder(name: string): Promise<void>;
  /** Перечитать статус истории в шапке. */
  refreshSync(): Promise<void>;
  /** Забрать и отправить историю на GitHub сейчас. */
  sync(): Promise<void>;
}

let page: Page = "general";
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

// Строка с выбором (ChoiceRow на маке): под заголовком — что выбрано (и пояснение через «·»), справа — «изменить» с меню.
const row = (title: string, subtitle: string, trailing: string) => {
  const chosen = /data-label="([^"]*)"/.exec(trailing)?.[1];
  const sub = chosen !== undefined ? (subtitle ? `<span>${chosen}</span> · <span>${subtitle}</span>` : chosen) : subtitle;
  return `
  <div class="setting"><div><b>${title}</b>${sub ? `<small>${sub}</small>` : ""}</div>${trailing}</div>`;
};
const pill = (act: string, text: string) => `<button class="pill" data-set="${act}">${text}</button>`;
const toggle = (key: string, on: boolean) => `<button class="switch${on ? " on" : ""}" data-toggle="${key}" role="switch" aria-checked="${on}"><i></i></button>`;
const menus = new Map<string, [string | number, string][]>();
const choice = (key: string, value: string | number, options: [string | number, string][]) => {
  menus.set(key, options);
  const label = options.find(([v]) => v === value)?.[1] ?? String(value);
  return `<button class="pill" data-menu="${key}" data-label="${label}">изменить</button>`;
};
const granted = (text: string) => `<span class="granted">${icon("taskDone")}${text}</span>`;
const later = (text = "переезжает с мака") => `<span class="later">${text}</span>`;
const note = (text: string) => `<p class="setting-note">${text}</p>`;

export async function mountSettings(root: HTMLElement, sidebar: HTMLElement, host: SettingsHost) {
  let prefs = await host.backend.prefs();
  const vault = await host.backend.vaultPath();
  let git = await host.backend.gitStatus();
  const templateCount = (await host.backend.templates().catch(() => [] as string[])).length;
  const folders = (await host.backend.tree().catch(() => null))?.folders.map((f) => f.name) ?? [];
  const { invoke } = await import("@tauri-apps/api/core");
  type ModelInfo = { id: string; installed: boolean; download_mb: number };
  let models: ModelInfo[] = host.backend.inTauri ? await invoke<ModelInfo[]>("models") : [];
  let downloading: number | null = null;
  let downloadingId = "";
  let recording = false;
  let autostartOn = host.backend.inTauri ? await invoke<boolean>("autostart") : false;
  let speakers = host.backend.inTauri ? await invoke<boolean>("speakers_installed") : false;
  let meaning = host.backend.inTauri ? await invoke<boolean>("meaning_installed") : false;
  let lock: LockState | null = host.backend.inTauri ? await lockApi.state() : null;
  let gh = await host.backend.githubStatus();
  // Вход по коду: код на экране, пока ждём подтверждения на github.com.
  let login: { code: string; url: string; error?: string } | null = null;
  const intelligence = host.backend.inTauri ? await invoke<{supported: boolean; reason: string | null}>("intelligence_state") : { supported: false, reason: null };
  const isMac = document.body.classList.contains("tauri-macos");

  function body(): string {
    switch (page) {
      case "general":
        return row("тема", "", choice("theme", prefs.theme ?? "system", [["system", "как в системе"], ["light", "светлая"], ["dark", "тёмная"]]))
          + row("язык", "", choice("uiLanguage", prefs.uiLanguage ?? "system", [["system", "как в системе"], ["ru", "Русский"], ["en", "English"], ["sr", "Srpski"]]))
          + row(os("открывать при входе в windows", "открывать при входе в mac"), "чтобы хоткей надиктовки работал сразу",
            host.backend.inTauri ? toggle("autostart", autostartOn) : later())
          + row("размер текста заметок", "", choice("textSize", prefs.textSize ?? 15, [13, 15, 17, 19].map((n) => [n, String(n)])))
          + row("проверка орфографии", "подчёркивать слова с ошибками", toggle("spellcheck", prefs.spellcheck ?? true))
          + (intelligence.supported ? row("apple intelligence", intelligence.reason ?? "кратко для заметок, на этом mac", intelligence.reason ? "" : toggle("intelligence", prefs.intelligence ?? false)) : "")
          + note(os("настройки хранятся на этом компьютере, в заметки не попадают", "настройки хранятся на этом mac, в заметки не попадают"));
      case "sync": {
        // Как SyncPage на маке: хранилище, история, github, как часто, последний коммит и отправка.
        const every = (m: number) => m === 1440 ? "раз в сутки" : m === 10080 ? "раз в неделю" : m % 60 === 0 ? (m === 60 ? "раз в час" : `раз в ${m / 60} ч`) : `раз в ${m} мин`;
        const date = (s: number) => { const d = new Date(s * 1000), t = d.toLocaleTimeString("ru", { hour: "2-digit", minute: "2-digit" }), y = new Date(Date.now() - 864e5);
          return d.toDateString() === new Date().toDateString() ? `сегодня в ${t}` : d.toDateString() === y.toDateString() ? `вчера в ${t}` : `${d.toLocaleDateString("ru", { day: "numeric", month: "short" })} в ${t}`; };
        const home = (p: string) => p.replace(/^\/Users\/[^/]+/, "~").replace(/^[A-Za-z]:\\Users\\[^\\]+/, "~");
        return row("папка хранилища", esc(vault ? home(vault) : "не выбрана"), pill("vault", "изменить"))
          + row("история в git", "каждая правка остаётся в истории", git.repo ? granted("включено") : pill("git", "включить"))
          + githubRows()
          + row("сохранять версию", "", choice("commitMinutes", prefs.commitMinutes ?? 60, [15, 30, 60, 180].map((m) => [m, every(m)])))
          + row("отправлять на github", "", choice("pushMinutes", prefs.pushMinutes ?? 1440, [60, 360, 1440, 10080].map((m) => [m, every(m)])))
          + row("последний коммит", git.last_commit ? date(git.last_commit) : "с запуска правок не было", "")
          + row("последняя отправка", prefs.lastSync ? date(Math.floor(prefs.lastSync / 1000)) : "ещё не отправляли", gh.signed_in ? pill("gh-sync", "отправить сейчас") : "")
          + note(os("правки с двух компьютеров сохранятся обе", "правки с двух mac сохранятся обе"));
      }
      case "search": {
        const status = !host.backend.inTauri ? later("в браузере нет")
          : downloading !== null ? `<span class="later">${Math.round(downloading * 100)}%</span>`
          : meaning ? granted("скачана") : pill("meaning-get", "скачать 471 мб");
        return row("поиск", os("ctrl+k из любого места", "⌘K из любого места"), granted("работает"))
          + row("по смыслу", "multilingual-e5-small: находит заметку, даже если слов из запроса в ней нет", status)
          + note("находит заметки, задачи, события календаря, видео и слова внутри расшифровок");
      }
      case "templates":
        return row("папка шаблонов", templateCount ? `${templateCount} ${templateCount % 10 === 1 && templateCount % 100 !== 11 ? "шаблон" : [2, 3, 4].includes(templateCount % 10) && ![12, 13, 14].includes(templateCount % 100) ? "шаблона" : "шаблонов"}` : "пока пусто, положи сюда .md", pill("templates", "открыть"))
          + note("в шаблоне работают {{дата}}, {{время}} и {{название}}")
          + row("заметка на каждый день", "в папке «дневник» по шаблону «день»", toggle("daily", prefs.daily ?? false));
      case "calendar":
        return row("календарь в приложении", "задачи с 📅 из заметок: день, неделя, месяц", granted("включено"))
          + row(isMac ? "календарь apple" : "календарь windows и outlook", isMac ? "переезжает с мака" : "файл .ics со всеми задачами со сроком", isMac ? later() : pill("ics", "сохранить .ics"))
          + `<div class="caption-line">задачи из папок</div>`
          + folders.map((f) => row(esc(f), "задачи этой папки — в задачах и календаре", toggle(`folder:${f}`, !(prefs.excludedFolders ?? []).includes(f)))).join("")
          + note(isMac ? "" : "системный календарь windows закрыт для приложений не из store — поэтому .ics");
      case "dictation": {
        // Как DictationPage на маке: хоткей, язык, вставка, длина, модели строками, спикеры.
        const mac = (c: string) => isMac ? c.split("+").map((k) => ({ ctrl: "⌃", alt: "⌥", shift: "⇧", super: "⌘" } as Record<string, string>)[k] ?? k).join(" ") : c.split("+").join(" + ");
        const hotkey = recording ? "…" : mac(prefs.hotkey ?? (isMac ? "alt+space" : "ctrl+alt+space"));
        const chosenModel = prefs.speechModel ?? "gigaam-v3-punct";
        const modelRow = (id: string, title: string, kind: string) => {
          const m = models.find((x) => x.id === id);
          const mb = m?.download_mb ?? 0;
          const size = mb >= 1000 ? `${(mb / 1000).toFixed(1).replace(".", ",")} гб` : `${mb} мб`;
          const status = !host.backend.inTauri ? later("в браузере нет")
            : id === chosenModel && m?.installed ? granted("выбрана")
            : downloading !== null && downloadingId === id ? `<span class="later">${Math.round(downloading * 100)}%</span>`
            : m?.installed ? pill(`model-use:${id}`, "выбрать") : pill(`model-get:${id}`, "скачать");
          return row(title, `${kind}, ${size}${m?.installed ? " · скачано" : ""}`, status);
        };
        const len = (m: number) => m === 0 ? "без предела" : `до ${m} мин`;
        return row("горячая клавиша", recording ? "нажми сочетание или esc" : "надиктовка из любого приложения",
            `<kbd class="keycap${recording ? " recording" : ""}">${hotkey}</kbd>${host.backend.inTauri ? `<button class="quiet-pill" data-set="hotkey">${recording ? "отмена" : "изменить"}</button>` : ""}`)
          + row("язык речи", "на каком языке говоришь", choice("language", prefs.language ?? "ru", [["auto", "определять сам"], ["ru", "русский"], ["en", "английский"], ["uk", "украинский"], ["de", "немецкий"], ["fr", "французский"], ["es", "испанский"]]))
          + row("вставлять в поле", "текст печатается сразу, копия остаётся в буфере", host.backend.inTauri ? toggle("paste", prefs.paste ?? false) : later())
          + row("длина записи", "дольше — запись остановится сама", choice("maxMinutes", prefs.maxMinutes ?? 0, [0, 1, 3, 5, 10, 15, 30, 60].map((m) => [m, len(m)])))
          + modelRow("gigaam-v3-punct", "GigaAM v3", "только русский, сразу с пунктуацией")
          + modelRow("whisper-small", "Whisper small", "99 языков")
          + modelRow("whisper-large-v3-turbo", "Whisper large-v3 turbo", "99 языков")
          + row("спикеры в расшифровках", "кто говорит в видео и аудио, 35 мб", !host.backend.inTauri ? later("в браузере нет")
            : speakers ? granted("скачаны") : pill("speakers-get", "скачать"))
          + note(os("всё распознаётся на этом компьютере, без интернета", "всё распознаётся на этом mac, без интернета"));
      }
      case "lock":
        if (!lock) return row("пинкод", "в окне приложения", later("в браузере нет"));
        return row("пинкод", lock.has_pin ? "4 цифры" : "не задан", pill("pin-on", lock.has_pin ? "сменить" : "задать"))
          + row(hello, os("лицо или палец вместо пинкода", "открывать отпечатком"), isMac ? `<span class="later">нет touch id</span>`
            : lock.hello ? granted("включено") : `<span class="later">${lock.has_pin ? "нет windows hello" : "сначала пинкод"}</span>`)
          + row("когда спрашивать", os("после блокировки компьютера или 15 минут без дела", "после блокировки mac или 15 минут без дела"), "")
          + note("пинкод закрывает приложение, файлы заметок не шифруются");
    }
  }

  function githubRows(): string {
    if (login) {
      return row("код для github", login.error ? esc(login.error) : "введите его на github.com/login/device — жду подтверждения…",
        `<span class="device-code">${esc(login.code)}</span>${pill("gh-open", "скопировать и открыть")}${pill("gh-cancel", "отмена")}`);
    }
    if (!gh.signed_in) return row("github", "войди — заметки будут и на github", pill("gh-login", "войти"));
    return row("github", "приватный репозиторий notes", granted("подключено"));
  }

  function render() {
    const [, title, subtitle] = PAGES.find(([p]) => p === page)!;
    root.innerHTML = `<div class="settings"><h1>${title}</h1><p class="settings-sub">${subtitle}</p><div class="settings-rows">${body()}</div></div>`;
    sidebar.innerHTML = PAGES.map(([p, t, , g]) =>
      `<button class="row${p === page ? " selected" : ""}" style="padding-left:8px" data-page="${p}">${icon(g)}<span>${t}</span></button>`).join("");
    host.onCrumb(`настройки / ${title}`);
  }

  async function save(next: Prefs) {
    prefs = next;
    await host.backend.setPrefs(prefs);
    host.apply(prefs);
    render();
  }

  root.addEventListener("click", async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    if (b.dataset.toggle?.startsWith("folder:")) {
      const f = b.dataset.toggle.slice(7);
      const off = new Set(prefs.excludedFolders ?? []);
      if (off.has(f)) off.delete(f); else off.add(f);
      await save({ ...prefs, excludedFolders: [...off] });
      return;
    }
    if (b.dataset.toggle === "autostart") {
      await invoke("set_autostart", { on: !autostartOn }).catch(() => {});
      autostartOn = await invoke<boolean>("autostart");
      render();
      return;
    }
    if (b.dataset.toggle) {
      const key = b.dataset.toggle as "spellcheck" | "daily" | "paste";
      const current = prefs[key] ?? (key === "spellcheck");
      await save({ ...prefs, [key]: !current });
    } else if (b.dataset.menu) {
      const key = b.dataset.menu as "theme" | "textSize" | "commitMinutes" | "speechModel" | "language" | "uiLanguage" | "maxMinutes" | "pushMinutes";
      const textual = key === "theme" || key === "speechModel" || key === "language" || key === "uiLanguage";
      const r = b.getBoundingClientRect();
      showMenu(r.left, r.bottom + 4, (menus.get(key) ?? []).map(([v, t]) => ({ label: t, run: () => void save({ ...prefs, [key]: textual ? v : Number(v) }) })));
    } else if (b.dataset.choice) {
      const key = b.dataset.choice as "theme" | "textSize" | "commitMinutes" | "speechModel" | "language" | "uiLanguage" | "maxMinutes" | "pushMinutes";
      const textual = key === "theme" || key === "speechModel" || key === "language" || key === "uiLanguage";
      await save({ ...prefs, [key]: textual ? b.dataset.value : Number(b.dataset.value) });
    } else if (b.dataset.set === "git") {
      b.textContent = "включаем…";
      await host.backend.gitEnable();
      git = await host.backend.gitStatus();
      await host.refreshSync();
      render();
    } else if (b.dataset.set === "commit") {
      const message = await host.backend.gitCommit();
      git = await host.backend.gitStatus();
      await host.refreshSync();
      render();
      const note = root.querySelector(".settings-rows");
      note?.insertAdjacentHTML("beforeend", `<p class="setting-note">${message ? esc(message) : "менять было нечего"}</p>`);
    } else if (b.dataset.set?.startsWith("model-get")) {
      downloadingId = b.dataset.set.split(":")[1] ?? "gigaam-v3-punct";
      const { listen } = await import("@tauri-apps/api/event");
      downloading = 0;
      render();
      const stop = await listen<number>("model-progress", (e) => {
        // Перерисовка раз в процент, не на каждый пакет.
        if (Math.round(e.payload * 100) !== Math.round((downloading ?? 0) * 100)) { downloading = e.payload; render(); }
      });
      try { await invoke("model_download", { id: downloadingId }); } catch (error) {
        root.querySelector(".settings-rows")?.insertAdjacentHTML("beforeend", note(`не скачалось: ${esc(String(error))}`));
      }
      stop();
      downloading = null;
      models = await invoke<ModelInfo[]>("models");
      render();
    } else if (b.dataset.set === "meaning-get") {
      const { listen } = await import("@tauri-apps/api/event");
      downloading = 0;
      render();
      const stop = await listen<number>("meaning-progress", (e) => {
        // Перерисовка раз в процент, не на каждый пакет.
        if (Math.round(e.payload * 100) !== Math.round((downloading ?? 0) * 100)) { downloading = e.payload; render(); }
      });
      try { await invoke("meaning_download"); } catch (error) {
        root.querySelector(".settings-rows")?.insertAdjacentHTML("beforeend", note(`не скачалось: ${esc(String(error))}`));
      }
      stop();
      downloading = null;
      meaning = await invoke<boolean>("meaning_installed");
      render();
    } else if (b.dataset.set?.startsWith("model-use:")) {
      await save({ ...prefs, speechModel: b.dataset.set.slice(10) });
    } else if (b.dataset.set === "hotkey") {
      // Следующее нажатие с Ctrl/Alt/Win (или F-клавиша) — новый хоткей; Esc — отмена.
      recording = true;
      render();
      const onKey = async (k: KeyboardEvent) => {
        k.preventDefault(); k.stopPropagation();
        if (k.key === "Escape") { done(); return; }
        if (["Control", "Alt", "Shift", "Meta"].includes(k.key)) return;
        const name = k.code === "Space" ? "space" : k.code.replace(/^Key|^Digit/, "").toLowerCase();
        const combo = [k.ctrlKey && "ctrl", k.altKey && "alt", k.shiftKey && "shift", k.metaKey && "super", name].filter(Boolean).join("+");
        try {
          await invoke("set_hotkey", { combo });
          await host.backend.setPrefs({ ...prefs, hotkey: combo });
          prefs = { ...prefs, hotkey: combo };
          done();
        } catch (error) { done(); root.querySelector(".settings-rows")?.insertAdjacentHTML("beforeend", note(esc(String(error)))); }
      };
      const done = () => { recording = false; removeEventListener("keydown", onKey, true); render(); };
      addEventListener("keydown", onKey, true);
    } else if (b.dataset.set === "speakers-get") {
      b.textContent = "качаем…";
      try { await invoke("speakers_download"); } catch (error) {
        root.querySelector(".settings-rows")?.insertAdjacentHTML("beforeend", note(`не скачалось: ${esc(String(error))}`));
      }
      speakers = await invoke<boolean>("speakers_installed");
      render();
    } else if (b.dataset.set?.startsWith("model-remove")) {
      await invoke("model_remove", { id: b.dataset.set.split(":")[1] });
      models = await invoke<ModelInfo[]>("models");
      render();
    } else if (b.dataset.set === "pin-on") {
      if (await choosePin()) { lock = await lockApi.state(); await lockApi.seen(); render(); }
    } else if (b.dataset.set === "pin-off") {
      await lockApi.clear();
      lock = await lockApi.state();
      render();
    } else if (b.dataset.set === "gh-login") {
      if (!git.repo) { await host.backend.gitEnable(); git = await host.backend.gitStatus(); }
      try {
        const code = await host.backend.githubCode();
        login = { code: code.user_code, url: code.verification_uri };
        render();
        const repo = await host.backend.githubFinish(code);
        login = null;
        gh = await host.backend.githubStatus();
        await save({ ...prefs, lastSync: Date.now() });
        root.querySelector(".settings-rows")?.insertAdjacentHTML("beforeend", note(`подключено: ${esc(repo)}`));
      } catch (error) {
        if (login && String(error) !== "cancelled") { login.error = String(error); render(); } else { login = null; render(); }
      }
      await host.refreshSync();
    } else if (b.dataset.set === "gh-open" && login) {
      await navigator.clipboard?.writeText(login.code).catch(() => {});
      await host.backend.openGithub(login.url);
    } else if (b.dataset.set === "gh-cancel") {
      login = null;
      await host.backend.githubCancel();
      render();
    } else if (b.dataset.set === "gh-out") {
      await host.backend.githubSignOut();
      gh = await host.backend.githubStatus();
      render();
      await host.refreshSync();
    } else if (b.dataset.set === "gh-sync") {
      b.textContent = "синхронизируем…";
      await host.sync();
      prefs = await host.backend.prefs();
      git = await host.backend.gitStatus();
      render();
    } else if (b.dataset.set === "vault") {
      await host.pickVault();
    } else if (b.dataset.set === "templates") {
      await host.ensureFolder("шаблоны");
      if (host.backend.inTauri) await host.backend.reveal("шаблоны");
    } else if (b.dataset.set === "ics") {
      if (await host.backend.exportIcs()) { b.textContent = "сохранено"; setTimeout(() => (b.textContent = "сохранить .ics"), 1600); }
    }
  });
  sidebar.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-page]");
    if (!b || !root.isConnected) return;
    page = b.dataset.page as Page;
    blurIn(root);
    render();
  });
  render();
}
