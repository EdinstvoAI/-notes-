// Настройки — порт `SettingsScreen.swift`: страницы в боковой панели, заголовок и серая подпись,
// стеклянные строки 640 с кнопкой справа. Здесь — то, что уже работает на Windows; остальное
// (git и GitHub, надиктовка, пинкод) — отдельной строкой «переезжает с мака», без выдуманного вида.

import type { Api, Prefs } from "./api";
import { icon, type Glyph } from "./icons";
import { choosePin, lockApi, type LockState } from "./lock";

type Page = "general" | "sync" | "search" | "templates" | "calendar" | "dictation" | "lock";
const PAGES: [Page, string, string, Glyph][] = [
  ["general", "общие", "вид, запуск и текст заметок", "settings"],
  ["sync", "синхронизация", "где лежат заметки и куда уходят", "git"],
  ["search", "поиск", "по словам и по смыслу", "search"],
  ["templates", "шаблоны", "шаблоны заметок и автоматизация", "file"],
  ["calendar", "календарь", "задачи со сроком в календаре", "calendar"],
  ["dictation", "надиктовка", "текст сразу в буфере", "microphone"],
  ["lock", "вход", "пинкод и windows hello", "task"],
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
const plural = (n: number, one: string, few: string, many: string) => {
  const d = n % 10, h = n % 100;
  return d === 1 && h !== 11 ? one : d >= 2 && d <= 4 && (h < 12 || h > 14) ? few : many;
};
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

const row = (title: string, subtitle: string, trailing: string) => `
  <div class="setting"><div><b>${title}</b><small>${subtitle}</small></div>${trailing}</div>`;
const pill = (act: string, text: string) => `<button class="pill" data-set="${act}">${text}</button>`;
const toggle = (key: string, on: boolean) => `<button class="switch${on ? " on" : ""}" data-toggle="${key}" role="switch" aria-checked="${on}"><i></i></button>`;
const choice = (key: string, value: string | number, options: [string | number, string][]) =>
  `<div class="segmented">${options.map(([v, t]) => `<button class="${v === value ? "on" : ""}" data-choice="${key}" data-value="${v}">${t}</button>`).join("")}</div>`;
const granted = (text: string) => `<span class="granted">${icon("taskDone")}${text}</span>`;
const later = (text = "переезжает с мака") => `<span class="later">${text}</span>`;
const note = (text: string) => `<p class="setting-note">${text}</p>`;

export async function mountSettings(root: HTMLElement, sidebar: HTMLElement, host: SettingsHost) {
  let prefs = await host.backend.prefs();
  const vault = await host.backend.vaultPath();
  let git = await host.backend.gitStatus();
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
  const ago = (s: number | null) => {
    if (s === null) return "ещё не сохраняли";
    const m = Math.floor((Date.now() / 1000 - s) / 60);
    return m < 1 ? "только что" : m < 60 ? `${m} мин назад` : m < 1440 ? `${Math.floor(m / 60)} ч назад` : new Date(s * 1000).toLocaleDateString("ru", { day: "numeric", month: "short" });
  };
  const isMac = document.body.classList.contains("tauri-macos");

  function body(): string {
    switch (page) {
      case "general":
        return row("тема", "как в системе, светлая или тёмная", choice("theme", prefs.theme ?? "system", [["system", "система"], ["light", "светлая"], ["dark", "тёмная"]]))
          + row("размер текста заметок", `${prefs.textSize ?? 15} — заголовки растут вместе с текстом`, choice("textSize", prefs.textSize ?? 15, [13, 15, 17, 19].map((n) => [n, String(n)])))
          + row("язык интерфейса", "как в системе, русский, english, srpski", choice("uiLanguage", prefs.uiLanguage ?? "system", [["system", "система"], ["ru", "ru"], ["en", "en"], ["sr", "sr"]]))
          + row("проверка орфографии", "подчёркивать слова с ошибкой", toggle("spellcheck", prefs.spellcheck ?? true))
          + row("открывать при входе в " + (isMac ? "mac" : "windows"), "сразу после включения компьютера",
            host.backend.inTauri && !isMac ? toggle("autostart", autostartOn) : later());
      case "sync":
        return row("папка хранилища", esc(vault ?? "не выбрана"), pill("vault", "изменить"))
          + row("история в git", git.repo ? `${git.commits} ${plural(git.commits, "версия", "версии", "версий")} · любую правку можно откатить` : "версии сохраняются по расписанию",
            git.repo ? granted("включено") : pill("git", "включить"))
          + (git.repo ? row("сохранять версию", "", choice("commitMinutes", prefs.commitMinutes ?? 60, [[15, "15 мин"], [30, "30 мин"], [60, "час"], [180, "3 ч"]]))
            + row("последняя версия", ago(git.last_commit) + (git.pending ? ` · несохранённых файлов: ${git.pending}` : ""), pill("commit", "сохранить сейчас")) : "")
          + githubRows()
          + note("папка та же, что на маке: заметки — обычные файлы .md, окончания строк не меняются");
      case "search": {
        const status = !host.backend.inTauri ? later("в браузере нет")
          : downloading !== null ? `<span class="later">${Math.round(downloading * 100)}%</span>`
          : meaning ? granted("скачана") : pill("meaning-get", "скачать 471 мб");
        return row("по словам", "заметки, задачи — всегда, без модели", granted("включено"))
          + row("по смыслу", "multilingual-e5-small: находит заметку, даже если слов из запроса в ней нет", status)
          + note("модель работает на этом компьютере, индекс — в данных приложения, не в папке заметок");
      }
      case "templates":
        return row("папка шаблонов", "обычные .md в папке «шаблоны»: {{дата}}, {{время}}, {{название}}", pill("templates", "открыть"))
          + row("заметка на каждый день", "в папке «дневник» по шаблону «день», открывается при запуске", toggle("daily", prefs.daily ?? false));
      case "calendar":
        return row("календарь в приложении", "задачи с 📅 из заметок: день, неделя, месяц", granted("включено"))
          + row(isMac ? "календарь apple" : "календарь windows и outlook", isMac ? "переезжает с мака" : "файл .ics со всеми задачами со сроком", isMac ? later() : pill("ics", "сохранить .ics"))
          + note(isMac ? "" : "системный календарь windows закрыт для приложений не из store — поэтому .ics");
      case "dictation": {
        const hotkey = recording ? "нажмите сочетание…" : (prefs.hotkey ?? (isMac ? "alt+space" : "ctrl+alt+space")).split("+").join(" + ");
        const modelRow = (id: string, title: string, sub: string) => {
          const m = models.find((x) => x.id === id);
          const status = !host.backend.inTauri ? later("в браузере нет")
            : downloading !== null && downloadingId === id ? `<span class="later">${Math.round(downloading * 100)}%</span>`
            : m?.installed ? `${granted("скачана")}${pill(`model-remove:${id}`, "удалить")}` : pill(`model-get:${id}`, `скачать ${m?.download_mb ?? ""} мб`);
          return row(title, sub, status);
        };
        const chosenModel = prefs.speechModel ?? "gigaam-v3-punct";
        return row("горячая клавиша", "нажать, сказать, нажать ещё раз: текст в буфере", `<span class="device-code">${hotkey}</span>${host.backend.inTauri ? pill("hotkey", "изменить") : ""}`)
          + row("распознавать", "надиктовка и видео", choice("speechModel", chosenModel, [["gigaam-v3-punct", "gigaam"], ["whisper-small", "whisper"]]))
          + modelRow("gigaam-v3-punct", "gigaam", "русский с пунктуацией, на этом компьютере")
          + modelRow("whisper-small", "whisper small", "99 языков, на этом компьютере")
          + (chosenModel === "whisper-small" ? row("язык речи", "для whisper", choice("language", prefs.language ?? "ru", [["auto", "сам"], ["ru", "ru"], ["en", "en"], ["uk", "uk"], ["de", "de"], ["fr", "fr"], ["es", "es"]])) : "")
          + row("спикеры", "кто что сказал в расшифровке видео · 35 мб", !host.backend.inTauri ? later("в браузере нет")
            : speakers ? granted("скачаны") : pill("speakers-get", "скачать"))
          + row("вставлять в поле", "ctrl+v за вас в поле, где курсор; иначе — только буфер", host.backend.inTauri ? toggle("paste", prefs.paste ?? false) : later())
          + note("модели не входят в приложение: качаются один раз и проверяются по контрольной сумме");
      }
      case "lock":
        if (!lock) return row("пинкод", "в окне приложения", later("в браузере нет"));
        return row("пинкод", lock.has_pin ? "при запуске и после 15 минут без приложения" : "4 цифры при входе в приложение",
          lock.has_pin ? `${granted("включён")}${pill("pin-off", "убрать")}` : pill("pin-on", "задать"))
          + row(isMac ? "touch id" : "windows hello", isMac ? "переезжает с мака"
            : lock.hello ? "лицо или палец вместо пинкода — спросит при входе" : lock.has_pin ? "не настроен в windows: параметры → учётные записи → варианты входа" : "сначала задайте пинкод",
            !isMac && lock.hello ? granted("включено") : later(""))
          + note("заметки пинкодом не шифруются — их по-прежнему откроет любой редактор; пинкод закрывает приложение");
    }
  }

  function githubRows(): string {
    if (login) {
      return row("код для github", login.error ? esc(login.error) : "введите его на github.com/login/device — жду подтверждения…",
        `<span class="device-code">${esc(login.code)}</span>${pill("gh-open", "скопировать и открыть")}${pill("gh-cancel", "отмена")}`);
    }
    if (!gh.signed_in) return row("github", "приватный репозиторий notes, вход по коду без пароля", pill("gh-login", "войти"));
    const repo = gh.remote?.replace(/^https:\/\/github\.com\//, "").replace(/\.git$/, "") ?? "notes";
    return row("github", `${esc(repo)} · ${prefs.lastSync ? "отправлено " + ago(Math.floor(prefs.lastSync / 1000)) : "ещё не отправляли"}`, granted("подключено"))
      + row("синхронизировать сейчас", "забрать правки с другого компьютера и отправить свои", pill("gh-sync", "синхронизировать"))
      + row("выйти из github", "токен удалится из хранилища паролей", pill("gh-out", "выйти"));
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
    } else if (b.dataset.choice) {
      const key = b.dataset.choice as "theme" | "textSize" | "commitMinutes" | "speechModel" | "language" | "uiLanguage";
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
    render();
  });
  render();
}
