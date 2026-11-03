// Первый вход — порт `OnboardingSteps.swift` (docs/ONBOARDING.md): привет → git в папке → github? →
// код → готово → модели → пару моментиков → пароль → всё готово. Тот же лист, что у входа по пинкоду:
// заголовок, серая подпись, коралловая капсула и тихая ссылка под ней. Пропуски — как на маке.

import { invoke } from "@tauri-apps/api/core";
import type { Api, Folder } from "./api";
import { choosePin } from "./lock";
import { drawSlats } from "./slats";
import { os } from "./platform";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

// Первый вход — своё окно 560×440, как `Layout.window` на маке; после — обычное окно заметок.
async function onboardingWindow(on: boolean) {
  const w = getCurrentWindow();
  try {
    if (on) { await w.setMinSize(new LogicalSize(560, 440)); await w.setSize(new LogicalSize(560, 440)); }
    else { await w.setSize(new LogicalSize(1200, 760)); await w.setMinSize(new LogicalSize(870, 760)); }
    await w.center();
  } catch { /* в браузере без Tauri окна нет */ }
}

/** Загрузка (кадр «Загрузка», ONBOARDING.md §0): планки растут в ширину 4 → 20 без зазоров за 2,4 с,
 *  логотип проявляется (fade + scale), потом уезжает вверх, на место в шапке. */
function loadingScene(): Promise<void> {
  return new Promise((done) => {
    const box = document.createElement("div");
    box.className = "loading-scene";
    box.innerHTML = `<canvas></canvas><b class="loading-logo">&lt;notes&gt;</b>`;
    document.body.append(box);
    const canvas = box.querySelector("canvas")!;
    const dark = matchMedia("(prefers-color-scheme: dark)").matches || document.documentElement.dataset.theme === "dark";
    const ease = (x: number) => 1 - Math.pow(1 - x, 3);
    const start = performance.now();
    const frame = (now: number) => {
      const k = Math.min(1, (now - start) / 2400);
      drawSlats(canvas, innerWidth, innerHeight, dark, 4 + 16 * ease(k));
      if (k < 1) requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
    setTimeout(() => box.classList.add("logo-in"), 400);
    setTimeout(() => box.classList.add("logo-up"), 2400);
    setTimeout(() => { box.classList.add("gone"); setTimeout(() => box.remove(), 600); done(); }, 2900);
  });
}

type Step = "welcome" | "git" | "github" | "code" | "githubDone" | "models" | "permissions" | "password" | "done";
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export async function runOnboarding(backend: Api): Promise<Folder> {
  await onboardingWindow(true);
  if (!matchMedia("(prefers-reduced-motion: reduce)").matches) await loadingScene();
  return new Promise((resolve) => {
    const el = document.createElement("div");
    el.className = "unlock onboarding";
    // Шапка из планок высотой 66 и логотип в ней; лист наезжает на неё с 54.
    const head = document.createElement("div");
    head.className = "onboarding-head";
    head.innerHTML = `<canvas></canvas><b>&lt;notes&gt;</b>`;
    document.body.append(head, el);
    const dark = () => matchMedia("(prefers-color-scheme: dark)").matches || document.documentElement.dataset.theme === "dark";
    const paint = () => drawSlats(head.querySelector("canvas")!, innerWidth, 66, dark(), 20);
    paint(); addEventListener("resize", paint);
    document.getElementById("app")!.classList.add("locked");
    let step: Step = "welcome";
    let tree: Folder | null = null;
    let extra = "";
    let repo = "notes";

    // StepLayout на маке: всё по верху листа — назад 32, заголовок 42, текст 83, кнопка primaryY − 54, ссылка 356.
    const view = (title: string, text: string, primary?: [string, string], secondary?: [string, string], body = "", y = 343, back?: string) => `
      <div class="unlock-sheet">${back ? `<button class="quiet back" data-go="${back}">назад</button>` : ""}<h1>${title}</h1><p>${text}</p>${body}
        ${primary ? `<button class="coral-capsule" style="top:${y - 54}px" data-go="${primary[1]}">${primary[0]}</button>` : ""}
        ${secondary ? `<button class="quiet second" data-go="${secondary[1]}">${secondary[0]}</button>` : ""}
        ${extra && step !== "code" ? `<p class="error">${esc(extra)}</p>` : ""}</div>`;

    let first = true;
    const render = () => {
      // Смена шага — старое уходит в размытие, новое проявляется (как шаги на маке).
      if (!first) { el.classList.add("step-swap"); setTimeout(() => el.classList.remove("step-swap"), 30); }
      first = false;
      el.innerHTML = {
        welcome: () => view("привет!", "выбери папку, куда сохранять заметки", ["создать папку", "create"], ["выбрать папку", "pick"]),
        git: () => view("git в папке", "папка станет git-репозиторием: любую правку можно откатить", ["инициализировать", "git"], ["пропустить, настрою потом", "models"]),
        github: () => view("отправлять на github?", "история хранится в приватном репозитории на github", ["войти через github", "login"], ["нет, только на этом компьютере", "models"]),
        code: () => view("введи код на github", "github.com/login/device — код уже скопирован", undefined, undefined, `<div class="device-code big">${esc((extra || "····–····").replace("-", "–"))}</div>`, 343, "github"),
        githubDone: () => view("готово!", `создали приватный репозиторий ${esc(repo)}. история отправляется туда сама`, ["дальше", "models"]),
        models: () => view("модели", "надиктовка gigaam (русский, 176 мб) работает без интернета, скачивается один раз", ["скачать и дальше", "download"], ["позже, в настройках", "permissions"], "", 374),
        permissions: () => view("пару моментиков", os("микрофон: параметры windows → конфиденциальность → микрофон. надиктовка — ctrl + alt + space", "микрофон: системные настройки → конфиденциальность → микрофон. надиктовка — ⌥ space"), ["дальше", "password"], undefined, "", 374),
        password: () => view("установим пароль?", "4 цифры при входе. заметки не шифруются — их откроет любой редактор", ["задать пинкод", "pin"], ["без пароля", "done"], "", 359),
        done: () => view("всё готово", "заметки — обычные файлы .md в твоей папке", ["открыть заметки", "finish"]),
      }[step]();
    };

    const go = async (to: string) => {
      extra = "";
      try {
        if (to === "pick" || to === "create") {
          if (to === "pick") tree = await backend.pickVault();
          else {
            // Как на маке: окно сохранения, имя по умолчанию «notes», папка создаётся там.
            const { save } = await import("@tauri-apps/plugin-dialog");
            const path = await save({ defaultPath: "notes", title: "Новая папка для заметок. Обычные файлы, открываются любым редактором." });
            tree = path ? await invoke<Folder>("create_vault_at", { path }) : null;
          }
          if (!tree) return;
          // Папка уже git-репозиторий — не предлагаем «инициализировать»: открываем его как есть
          // (история и .gitignore не трогаются), а с готовым github — сразу к моделям.
          if ((await backend.gitStatus()).repo) {
            await backend.gitEnable();
            const gh = await backend.githubStatus().catch(() => null);
            step = gh?.signed_in && gh.remote ? "models" : "github";
          } else step = "git";
        } else if (to === "git") {
          await backend.gitEnable();
          step = "github";
        } else if (to === "login") {
          const code = await backend.githubCode();
          await navigator.clipboard?.writeText(code.user_code).catch(() => {});
          await backend.openGithub(code.verification_uri).catch(() => {});
          step = "code"; extra = code.user_code; render();
          repo = await backend.githubFinish(code);
          step = "githubDone";
        } else if (to === "github" && step === "code") {
          await backend.githubCancel();
          step = "github";
        } else if (to === "download") {
          step = "permissions";
          // Модель качается в фоне — не держим человека на экране, как шаг 7 на маке не ждёт конца.
          void invoke("model_download").catch(() => {});
        } else if (to === "pin") {
          if (await choosePin()) step = "done";
        } else if (to === "finish") {
          el.classList.add("leaving"); head.classList.add("leaving");
          document.getElementById("app")!.classList.remove("locked");
          removeEventListener("resize", paint);
          setTimeout(() => { el.remove(); head.remove(); void onboardingWindow(false); }, 450);
          resolve(tree!);
          return;
        } else step = to as Step;
      } catch (e) {
        if (String(e) !== "cancelled") extra = String(e);
      }
      render();
    };
    el.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>("[data-go]");
      if (b) void go(b.dataset.go!);
    });
    render();
    requestAnimationFrame(() => el.classList.add("shown"));
  });
}
