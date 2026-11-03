// Английский и сербский интерфейс — переводы из каталога мака (`<notes>/Localizable.xcstrings`, en и
// sr-Latn), чтобы слова совпадали на обеих ОС. Исходный язык — русский. Переводим готовый экран:
// текст и подсказки элементов, кроме того, что написал человек (заметки, имена, расшифровки).

import table from "./i18n.json";

type Lang = "ru" | "en" | "sr";
const dict = table as unknown as Record<string, { en: string; sr: string }>;
// Строки с числами: шаблоны из каталога мака (%lld, %@ → $1, $2) плюс свои строки Windows-версии.
const patterns: [RegExp, string, string][] = [
  ...((table as unknown as { __patterns: [string, string, string][] }).__patterns ?? []).map(([r, en, sr]) => [new RegExp(r), en, sr] as [RegExp, string, string]),
  [/^сохранено только что$/, "saved just now", "sačuvano upravo"],
  [/^сохранено (\d+) мин назад$/, "saved $1 min ago", "sačuvano pre $1 min"],
  [/^сохранено (\d+) ч назад$/, "saved $1 h ago", "sačuvano pre $1 h"],
  [/^синхронизировано только что$/, "synced just now", "sinhronizovano upravo"],
  [/^(\d+) пап(?:ка|ки|ок) \| (\d+) замет(?:ка|ки|ок)$/, "$1 folders | $2 notes", "fascikli: $1 | beleške: $2"],
  [/^(\d+) замет(?:ка|ки|ок)$/, "$1 notes", "beleške: $1"],
  [/^изменено сегодня в (\S+) · (\d+) сл(?:ово|ова|ов)$/, "edited today at $1 · $2 words", "izmenjeno danas u $1 · reči: $2"],
  [/^изменено (.+) в (\S+) · (\d+) сл(?:ово|ова|ов)$/, "edited $1 at $2 · $3 words", "izmenjeno $1 u $2 · reči: $3"],
  [/^сегодня в (\S+)$/, "today at $1", "danas u $1"],
  [/^(\d+) открыто \| (\d+) с датой$/, "$1 open | $2 dated", "otvoreno: $1 | sa datumom: $2"],
];
let lang: Lang = "ru";
const USER = ".note-text, .note-title, .transcript, .task-title, .task-note, [data-note], [data-folder], [data-file], .card, .note-row, .tab, .result-head, .snippet, .device-code, input, textarea";

export function languageFor(pref: string | undefined): Lang {
  if (pref === "ru" || pref === "en" || pref === "sr") return pref;
  const sys = navigator.language.toLowerCase();
  return sys.startsWith("ru") ? "ru" : sys.startsWith("sr") ? "sr" : sys.startsWith("en") ? "en" : "ru";
}

const tr = (s: string) => {
  const key = s.trim();
  if (!key) return s;
  const hit = dict[key];
  if (hit && typeof hit === "object" && "en" in hit) return s.replace(key, hit[lang as "en" | "sr"]);
  for (const [re, en, sr] of patterns) {
    if (re.test(key)) return s.replace(key, key.replace(re, lang === "en" ? en : sr));
  }
  return s;
};

function translate(root: Node) {
  if (lang === "ru") return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  let n: Node | null = walker.currentNode;
  while (n) {
    if (n.nodeType === Node.TEXT_NODE) {
      const parent = n.parentElement;
      if (parent && !parent.closest(USER)) { const v = tr(n.nodeValue ?? ""); if (v !== n.nodeValue) n.nodeValue = v; }
    } else if (n instanceof HTMLElement && !n.closest(USER)) {
      for (const a of ["title", "placeholder", "aria-label"]) { const v = n.getAttribute(a); if (v) { const t = tr(v); if (t !== v) n.setAttribute(a, t); } }
    }
    n = walker.nextNode();
  }
}

let observer: MutationObserver | null = null;
export function setLanguage(next: Lang) {
  lang = next;
  document.documentElement.lang = next === "sr" ? "sr-Latn" : next;
  observer?.disconnect();
  if (next === "ru") return;
  translate(document.body);
  observer = new MutationObserver((list) => list.forEach((m) => {
    m.addedNodes.forEach(translate);
    if (m.type === "characterData" && m.target.parentElement && !m.target.parentElement.closest(USER)) {
      const v = tr(m.target.nodeValue ?? "");
      if (v !== m.target.nodeValue) m.target.nodeValue = v;
    }
  }));
  observer.observe(document.body, { childList: true, subtree: true, characterData: true });
}
