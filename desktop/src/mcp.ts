// Раздел «mcp» — порт `MCPScreen.swift` (03.11.2026): нейронки (Claude Code, Codex, Cursor) работают с
// заметками через `notes-desktop --mcp`, без сети. Подключение, закрытые папки, «только чтение», журнал
// обращений с оценкой токенов; «скиллы» — готовые инструкции, кладутся в папку скиллов нейронки.

import { invoke } from "@tauri-apps/api/core";
import type { Prefs } from "./api";
import { icon } from "./icons";
import { showMenu } from "./menu";

type Agent = "claude" | "codex" | "cursor";
const AGENTS: [Agent, string][] = [["claude", "Claude Code"], ["codex", "Codex"], ["cursor", "Cursor"]];

const SKILLS = [
  { name: "notes-assistant", title: "помощник по заметкам", about: "отвечает, опираясь на заметки, и ссылается на них",
    text: "У тебя есть MCP-сервер notes — мои Markdown-заметки. Прежде чем отвечать, ищи в них (search_notes) и читай найденное (read_note). Ссылайся на путь заметки. Новое записывай отдельной заметкой (create_note), старые не переписывай." },
  { name: "notes-inbox", title: "разбор входящих", about: "раскладывает «Входящие» на задачи и заметки",
    text: "Прочитай «Входящие.md» через MCP notes. Разложи мысли: дела — задачами (add_task, со сроком, если он есть), идеи — отдельными заметками в подходящие папки (create_note). В конце — короткий список, что куда ушло." },
  { name: "notes-planner", title: "планировщик задач", about: "собирает открытые задачи в план на день",
    text: "Через MCP notes возьми открытые задачи (list_tasks). Сгруппируй: сегодня, неделя, без срока. Предложи план на день — три главных дела. Новые задачи добавляй через add_task в «Входящие.md»." },
  { name: "notes-weekly", title: "итоги недели", about: "что сделано и что осталось — отдельной заметкой",
    text: "Через MCP notes посмотри заметки, изменённые за последние 7 дней (list_notes, свежие сверху), и задачи (list_tasks с include_done). Напиши итоги недели: что сделано, что осталось, что застряло. Сохрани новой заметкой «Итоги недели ГГГГ-ММ-ДД» (create_note)." },
  { name: "notes-meeting", title: "конспект встречи", about: "из сырого текста — решения, задачи и заметка",
    text: "Я даю сырой текст или расшифровку встречи. Выдели решения, открытые вопросы и задачи с ответственными и сроками. Сохрани конспект новой заметкой (create_note) в папку «Работа», задачи — через add_task в эту же заметку." },
  { name: "notes-links", title: "поиск связей", about: "находит заметки на ту же тему и предлагает связи",
    text: "Возьми заметку, которую я назову (read_note). Найди через search_notes заметки на ту же тему по ключевым словам. Перечисли связанные с путями и одной строкой — чем они связаны. Сами заметки не меняй." },
];

interface Info { exe: string; recent: [number, string, string][]; tokens: number }

export interface McpHost {
  inTauri: boolean;
  prefs(): Prefs;
  setPrefs(p: Prefs): Promise<void>;
  /** Папки верхнего уровня хранилища. */
  folders(): string[];
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const row = (title: string, sub: string, trailing: string) =>
  `<div class="setting"><div><b>${title}</b><small>${sub}</small></div>${trailing}</div>`;

let skillsPage = false;
let agent: Agent = "claude";

export function connectCommand(a: Agent, exe: string): string {
  // Кавычки — двойные: путь Windows с пробелами («Program Files») и PowerShell, и cmd понимают так.
  if (a === "claude") return `claude mcp add -s user notes -- "${exe}" --mcp`;
  if (a === "codex") return `codex mcp add notes -- "${exe}" --mcp`;
  return JSON.stringify({ mcpServers: { notes: { command: exe, args: ["--mcp"] } } });
}

export async function mountMcp(root: HTMLElement, sidebar: HTMLElement, host: McpHost) {
  let info: Info = { exe: "", recent: [], tokens: 0 };
  let installed: boolean[] = [];
  let copied = "";

  async function load() {
    if (host.inTauri) {
      info = await invoke<Info>("mcp_info");
      installed = agent === "cursor" ? [] : await invoke<boolean[]>("mcp_skills", { agent, names: SKILLS.map((s) => s.name) });
    }
    paint();
  }

  const copy = (key: string, label = "скопировать") => `<button class="pill" data-copy="${key}">${copied === key ? "скопировано" : label}</button>`;

  function paint() {
    sidebar.innerHTML = `
      <button class="row${skillsPage ? "" : " selected"}" style="padding-left:8px" data-mcp="connect">${icon("spark")}<span>подключение</span></button>
      <button class="row${skillsPage ? " selected" : ""}" style="padding-left:8px" data-mcp="skills">${icon("file")}<span>скиллы</span><small class="count">${SKILLS.length}</small></button>`;
    const p = host.prefs();
    const hidden = p.mcpHiddenFolders ?? [];
    const name = AGENTS.find(([a]) => a === agent)![1];
    const head = skillsPage
      ? `<h1>скиллы</h1><p class="settings-sub">готовые инструкции для нейросети: что делать с заметками</p>`
      : `<h1>mcp</h1><p class="settings-sub">локальный сервер: claude code, codex и cursor работают с заметками на этом компьютере, без сети</p>`;
    let body = row("нейросеть", name, `<button class="pill" data-agent>изменить</button>`);
    if (skillsPage) {
      body += SKILLS.map((s, i) => row(s.title, s.about, agent === "cursor" ? copy(`skill:${s.name}`)
        : `<button class="pill" data-install="${i}">${installed[i] ? "установлен" : "установить"}</button>`)).join("");
    } else {
      body += row("подключить", agent === "cursor" ? "вставь в %USERPROFILE%\\.cursor\\mcp.json" : "команда в терминал, один раз", copy("connect"))
        + row("закрытые папки", hidden.length ? esc([...hidden].sort().join(", ")) : "нет — видно всё хранилище", `<button class="pill" data-hidden>выбрать</button>`)
        + row("только чтение", "нейросеть ищет и читает, но не создаёт заметки и задачи",
          `<button class="switch${p.mcpReadOnly ? " on" : ""}" data-readonly role="switch" aria-checked="${!!p.mcpReadOnly}"><i></i></button>`)
        + `<div class="mcp-log-head"><div><b>последние обращения</b><small>≈ ${info.tokens.toLocaleString("ru")} токенов отдано нейросети</small></div>
            ${info.recent.length ? `<button class="quiet" data-clear>очистить</button>` : ""}</div>`
        + (info.recent.length ? info.recent.map(([t, tool, target]) => `<div class="mcp-log"><small>${new Date(t * 1000)
            .toLocaleString("ru", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }).replace(",", "")}</small>
            <span>${esc(tool)}</span><span class="dim">${esc(target)}</span></div>`).join("")
          : `<small class="mcp-empty">пока не было</small>`);
    }
    root.innerHTML = `<div class="settings">${head}<div class="settings-rows">${body}</div></div>`;
  }

  root.onclick = async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    const p = host.prefs();
    if (b.dataset.copy) {
      const key = b.dataset.copy;
      const text = key === "connect" ? connectCommand(agent, info.exe) : SKILLS.find((s) => `skill:${s.name}` === key)!.text;
      await navigator.clipboard.writeText(text).catch(() => {});
      copied = key; paint();
    } else if (b.dataset.agent !== undefined) {
      const r = b.getBoundingClientRect();
      showMenu(r.left, r.bottom + 4, AGENTS.map(([a, label]) => ({ label, run: () => { agent = a; copied = ""; void load(); } })));
    } else if (b.dataset.hidden !== undefined) {
      const r = b.getBoundingClientRect();
      const hidden = new Set(p.mcpHiddenFolders ?? []);
      const folders = host.folders();
      showMenu(r.left, r.bottom + 4, folders.length ? folders.map((f) => ({ label: `${hidden.has(f) ? "✓ " : ""}${f}`, run: async () => {
        if (hidden.has(f)) hidden.delete(f); else hidden.add(f);
        await host.setPrefs({ ...p, mcpHiddenFolders: [...hidden].sort() }); paint();
      } })) : [{ label: "папок нет", run: () => {} }]);
    } else if (b.dataset.readonly !== undefined) {
      await host.setPrefs({ ...p, mcpReadOnly: !p.mcpReadOnly }); paint();
    } else if (b.dataset.clear !== undefined) {
      await invoke("mcp_clear_log"); await load();
    } else if (b.dataset.install) {
      const s = SKILLS[Number(b.dataset.install)];
      await invoke("mcp_install_skill", { agent, name: s.name, title: s.title, about: s.about, text: s.text }).catch(() => {});
      await load();
    }
  };
  sidebar.onclick = (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-mcp]");
    if (!b || !root.isConnected) return;
    skillsPage = b.dataset.mcp === "skills"; copied = ""; paint();
  };
  // Журнал обновляется сам, пока раздел открыт (на маке — раз в 3 с).
  const timer = setInterval(() => { if (!root.isConnected) clearInterval(timer); else if (!skillsPage && host.inTauri) void load(); }, 3000);
  await load();
}
