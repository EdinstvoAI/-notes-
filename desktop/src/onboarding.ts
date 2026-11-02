// Первый вход — порт `OnboardingSteps.swift` (docs/ONBOARDING.md): привет → git в папке → github? →
// код → готово → модели → пару моментиков → пароль → всё готово. Тот же лист, что у входа по пинкоду:
// заголовок, серая подпись, коралловая капсула и тихая ссылка под ней. Пропуски — как на маке.

import { invoke } from "@tauri-apps/api/core";
import type { Api, Folder } from "./api";
import { choosePin } from "./lock";

type Step = "welcome" | "git" | "github" | "code" | "githubDone" | "models" | "permissions" | "password" | "done";
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export function runOnboarding(backend: Api): Promise<Folder> {
  return new Promise((resolve) => {
    const el = document.createElement("div");
    el.className = "unlock onboarding";
    document.body.append(el);
    document.getElementById("app")!.classList.add("locked");
    let step: Step = "welcome";
    let tree: Folder | null = null;
    let extra = "";
    let repo = "notes";

    const view = (title: string, text: string, primary?: [string, string], secondary?: [string, string], body = "") => `
      <div class="unlock-sheet"><h1>${title}</h1><p>${text}</p>${body}
        ${primary ? `<button class="coral-capsule" data-go="${primary[1]}">${primary[0]}</button>` : ""}
        ${secondary ? `<button class="quiet" data-go="${secondary[1]}">${secondary[0]}</button>` : ""}
        ${extra ? `<p class="error">${esc(extra)}</p>` : ""}</div>`;

    const render = () => {
      el.innerHTML = {
        welcome: () => view("привет!", "выбери папку, куда сохранять заметки", ["создать папку", "create"], ["выбрать папку", "pick"]),
        git: () => view("git в папке", "папка станет git-репозиторием: любую правку можно откатить", ["инициализировать", "git"], ["пропустить, настрою потом", "models"]),
        github: () => view("отправлять на github?", "история хранится в приватном репозитории на github", ["войти через github", "login"], ["нет, только на этом компьютере", "models"]),
        code: () => view("введи код на github", "github.com/login/device — код уже скопирован", undefined, ["назад", "github"], `<div class="device-code big">${esc(extra || "····–····")}</div>`),
        githubDone: () => view("готово!", `создали приватный репозиторий ${esc(repo)}. история отправляется туда сама`, ["дальше", "models"]),
        models: () => view("модели", "надиктовка gigaam (русский, 176 мб) работает без интернета, скачивается один раз", ["скачать и дальше", "download"], ["позже, в настройках", "permissions"]),
        permissions: () => view("пару моментиков", "микрофон: параметры windows → конфиденциальность → микрофон. надиктовка — ctrl + alt + space", ["дальше", "password"]),
        password: () => view("установим пароль?", "4 цифры при входе. заметки не шифруются — их откроет любой редактор", ["задать пинкод", "pin"], ["без пароля", "done"]),
        done: () => view("всё готово", "заметки — обычные файлы .md в твоей папке", ["открыть заметки", "finish"]),
      }[step]();
    };

    const go = async (to: string) => {
      extra = "";
      try {
        if (to === "pick" || to === "create") {
          tree = to === "pick" ? await backend.pickVault() : await invoke<Folder | null>("create_vault");
          if (!tree) return;
          step = "git";
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
          el.classList.add("leaving");
          document.getElementById("app")!.classList.remove("locked");
          setTimeout(() => el.remove(), 450);
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
