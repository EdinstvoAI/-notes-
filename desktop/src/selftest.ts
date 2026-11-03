// Самопроверка в настоящем окне Tauri (`NOTES_SELFTEST=1`), как `-notesSelfTest` на маке: сценарий идёт
// через настоящее Rust-ядро на настоящей папке — то, что Playwright на демо-хранилище не проверит.
// Хранилище — тестовое, не настоящее.

import { invoke } from "@tauri-apps/api/core";
import type { Api } from "./api";

export async function runSelftest(backend: Api, ui: { openNote(path: string): Promise<void>; app: HTMLElement }) {
  const log = (line: string, done = false) => invoke("selftest_log", { line, done });
  const results: string[] = [];
  const check = async (name: string, ok: boolean) => {
    results.push(`${ok ? "ok  " : "FAIL"} ${name}`);
    await log(`${ok ? "ok  " : "FAIL"} ${name}`);
    await pause(step);
  };
  const pause = (ms: number) => new Promise((r) => setTimeout(r, ms));
  // Для записи видео шаги видны глазом: NOTES_SELFTEST_SLOW=1 — пауза между проверками.
  const step = (await invoke<boolean>("selftest_slow")) ? 900 : 0;
  try {
    const tree = await backend.tree();
    await check("vault tree from Rust", tree.folders.some((f) => f.name === "проекты"));
    await ui.openNote("проекты/план.md");
    await pause(800);
    await check("note opened in editor", (ui.app.querySelector(".note-text")?.textContent ?? "").includes("проверить на Windows"));
    const note = await backend.create("проекты", "Новая заметка");
    await backend.write(note, "# Из самопроверки\n\nстрока\n");
    await check("write and read back", (await backend.read(note)).startsWith("# Из самопроверки"));
    const renamed = await backend.rename(note, "итоги");
    await check("rename", renamed === "проекты/итоги.md");
    const hits = await backend.search("самопроверки");
    await check("search finds it", hits.some((h) => h.note.path === renamed));
    await ui.openNote(renamed);
    const tasks = await backend.tasks();
    const t = tasks.find((x) => x.title === "купить хлеб");
    await check("tasks parsed", !!t && tasks.length >= 2);
    if (t) await backend.setDue(t, "2026-11-05", "10:00", 30);
    await check("set due writes the line", (await backend.read("Входящие.md")).includes("купить хлеб 📅 2026-11-05 10:00–10:30"));
    await backend.gitEnable();
    let s = await backend.gitStatus();
    await check("git enabled with first commit", s.repo && s.commits === 1);
    s = await backend.gitStatus();
    await backend.write(renamed, "# Из самопроверки\n\nправка\n");
    const message = await backend.gitCommit();
    s = await backend.gitStatus();
    await check(`commit «${message}»`, s.commits === 2 && message === "правка: итоги.md");
    // Синхронизация: версия в панели линией метро, отмена версии — новым коммитом, файл уходит.
    await backend.write("версия.md", "для отката\n");
    await backend.gitCommit();
    ui.app.querySelector<HTMLElement>('[data-section="sync"]')?.click();
    await pause(900);
    await check("sync section lists versions", ui.app.querySelectorAll(".metro-row").length >= 2 && !!ui.app.querySelector(".version-file"));
    const top = await invoke<{ id: string }[]>("git_history");
    await invoke("git_revert", { id: top[0].id });
    await check("revert a version", !(await backend.tree()).notes.some((n) => n.path === "версия.md")
      && (await invoke<{ message: string }[]>("git_history"))[0].message.startsWith("отменено:"));
    ui.app.querySelector<HTMLElement>('[data-sync="board"]')?.click();
    await pause(600);
    ui.app.querySelector<HTMLElement>(".board-row")?.click();
    await pause(400);
    await check("graph board with card", !!ui.app.querySelector(".board-card .version-file"));
    if (await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_HOLD" })) await pause(4000);
    // Сейф: свой мастер-пароль, запись, код 2FA по RFC 6238, файлы в .safe/ шифром, закрытие.
    ui.app.querySelector<HTMLElement>('[data-section="safe"]')?.click();
    await pause(700);
    await invoke("safe_create", { password: "Тест-пароль-2026" });
    await invoke("safe_put", { item: { id: "", kind: "login", title: "почта", username: "makar@example.com", secret: "тайна-1", url: "", note: "", modified: 0 } });
    await invoke("safe_put", { item: { id: "", kind: "totp", title: "github", username: "makark", secret: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", url: "", note: "", modified: 0 } });
    ui.app.querySelector<HTMLElement>('[data-section="notes"]')?.click();
    await pause(300);
    ui.app.querySelector<HTMLElement>('[data-section="safe"]')?.click();
    await pause(900);
    const [codes] = await invoke<[Record<string, string>, number]>("safe_codes");
    await check("safe: items, 2fa code, encrypted files", !!ui.app.querySelector(".safe-row") && Object.values(codes).some((c) => /^\d{6}$/.test(c))
      && !(await backend.tree()).notes.some((n) => n.path.startsWith(".safe")));
    ui.app.querySelector<HTMLElement>('[data-kind="totp"]')?.click();
    await pause(300);
    if (await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_HOLD" })) await pause(4000);
    ui.app.querySelector<HTMLElement>('[data-side="lock"]')?.click();
    await pause(500);
    await invoke("safe_unlock", { text: "не тот" }).catch(() => {});
    const locked = !(await invoke<{ unlocked: boolean }>("safe_state")).unlocked;
    await invoke("safe_unlock", { text: "Тест-пароль-2026" });
    await check("safe: lock, wrong key refused, reopen", locked && (await invoke<unknown[]>("safe_items")).length === 2);
    await invoke("safe_lock");
    // MCP: раздел, «только чтение» пишется в настройки — его прочтёт `--mcp`.
    ui.app.querySelector<HTMLElement>('[data-section="mcp"]')?.click();
    await pause(700);
    ui.app.querySelector<HTMLElement>("[data-readonly]")?.click();
    await pause(400);
    await check("mcp section, read-only saved", !!ui.app.querySelector('[data-copy="connect"]') && JSON.parse(await invoke<string>("prefs")).mcpReadOnly === true);
    if (await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_HOLD" })) await pause(4000);
    ui.app.querySelector<HTMLElement>("[data-readonly]")?.click();
    ui.app.querySelector<HTMLElement>('[data-section="notes"]')?.click();
    await pause(300);
    const dup = await backend.duplicate(renamed);
    const moved = await backend.moveInto(dup, "");
    await check("duplicate and move", moved === "итоги копия.md");
    await backend.trash(moved!);
    await check("trash", !(await backend.tree()).notes.some((n) => n.path === moved));
    await backend.untrash(moved!);
    await check("Ctrl+Z: back from trash", (await backend.tree()).notes.some((n) => n.path === moved));
    // Правки снаружи: файл появился в папке — панель узнаёт сама.
    await backend.write("снаружи.md", "из другого редактора\n");
    await pause(1500);
    await check("external change shows in sidebar", !!ui.app.querySelector('.sidebar [data-note="снаружи.md"]'));
    const daily = await backend.daily("2026-11-01", "09:00");
    await check("daily note", daily === "дневник/2026-11-01.md");
    await ui.openNote("Входящие.md");
    // Просмотр файла: картинка из хранилища показывается в листе.
    (ui.app.querySelector('.sidebar [data-folder="проекты"]') as HTMLElement | null)?.click();
    await pause(600);
    (ui.app.querySelector('.sidebar [data-file="проекты/иконка.png"]') as HTMLElement | null)?.click();
    await pause(1500);
    const img = ui.app.querySelector<HTMLImageElement>(".preview-body img");
    await check("image preview", !!img && img.naturalWidth === 128);
    // Хоткей: своё сочетание регистрируется, без Ctrl/Alt/Win — нет.
    const okKey = await invoke("set_hotkey", { combo: "ctrl+alt+d" }).then(() => true).catch(() => false);
    const badKey = await invoke("set_hotkey", { combo: "d" }).then(() => true).catch(() => false);
    await check("hotkey change rules", okKey && !badKey);
    // Пинкод: хранилище паролей ОС, экран входа, неверный и верный ввод клавишами.
    const { lockApi, showUnlock } = await import("./lock");
    await lockApi.set("1234");
    const lock = await lockApi.state();
    await check("pin saved in OS keyring", lock.has_pin);
    const unlocked = showUnlock({ ...lock, hello: false });
    await pause(400);
    const press = (k: string) => document.dispatchEvent(new KeyboardEvent("keydown", { key: k }));
    for (const k of "0000") press(k);
    await pause(1500);
    await check("wrong pin rejected", document.querySelector(".unlock .pin-cells.error") !== null);
    for (const k of "1234") press(k);
    await Promise.race([unlocked, pause(4000)]);
    await pause(600);
    await check("right pin unlocks", document.querySelector(".unlock") === null);
    await lockApi.clear();
    await check("pin cleared", !(await lockApi.state()).has_pin);
    // Поиск по смыслу: модель кладём из NOTES_SELFTEST_E5 (облако не качает с Hugging Face своим клиентом).
    const e5 = await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_E5" });
    if (e5) {
      await backend.write("проекты/счёт.md", "# счёт клиенту\n\nВыставить счёт студии за макеты лендинга, оплата до пятницы.\n");
      await invoke("selftest_install_e5", { from: e5 });
      const hits = await invoke<[string, number][]>("meaning_search", { query: "сколько мне заплатят за дизайн" });
      await check(`meaning search → ${hits[0]?.[0]}`, hits[0]?.[0] === "проекты/счёт.md");
    }
    // Надиктовка без микрофона: WAV модели проходит весь путь до буфера обмена (как -dictationTestFile).
    const archive = await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_GIGAAM" });
    if (archive) {
      const { listen } = await import("@tauri-apps/api/event");
      let copied = "";
      const states: string[] = [];
      const stop = await listen<{ state: string; text?: string }>("dictation", (e) => { states.push(e.payload.state); if (e.payload.text) copied = e.payload.text; });
      const wav = archive.replace(/[^/\\]+$/, "") + "sherpa-onnx-nemo-transducer-punct-giga-am-v3-russian-2025-12-16/test_wavs/example.wav";
      await invoke("dictation_test", { wav, archive });
      for (let i = 0; i < 120 && !copied; i++) await pause(500);
      stop();
      await check(`dictation → clipboard: «${copied.slice(0, 40)}…»`, copied.includes("лукоморья") && states.includes("recognizing"));
      // Видео: тот же WAV как файл → расшифровка в фоне → заметка в «расшифровки/».
      let done = false;
      const stopV = await listen<{ words: number | null }>("video", (e) => { if (e.payload.words) done = true; });
      const seg = await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_SEG" });
      const emb = await invoke<string | null>("selftest_env", { name: "NOTES_SELFTEST_EMB" });
      if (seg && emb) await invoke("selftest_install_speakers", { segmentation: seg, embedding: emb });
      const item = await invoke<{ id: string }>("video_add", { path: wav, added: "2026-11-02 12:00" });
      for (let i = 0; i < 120 && !done; i++) await pause(500);
      stopV();
      await invoke("video_folder", { id: item.id, folder: "созвоны" });
      const found = await invoke<[string, string, string][]>("video_search", { query: "ЛУКОМОРЬЯ дуб" });
      await check("search finds words in transcripts", found.some(([id]) => id === item.id));
      const note = await invoke<string>("video_save_note", { id: item.id });
      const text = await backend.read(note);
      await check(`transcript note ${note}`, note === "расшифровки/созвоны/example.md" && /\*\*00:00( спикер 1:)?\*\* /.test(text) && text.includes("модель: gigaam") && (!seg || text.includes("спикер 1:")));
    }
    await pause(300);
  } catch (e) {
    await check(`exception: ${e}`, false);
  }
  const failed = results.filter((r) => r.startsWith("FAIL")).length;
  await log(`${results.length - failed}/${results.length}`, true);
}
