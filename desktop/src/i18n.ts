// Английский и сербский интерфейс — переводы из каталога мака (`<notes>/Localizable.xcstrings`, en и
// sr-Latn), чтобы слова совпадали на обеих ОС. Исходный язык — русский. Переводим готовый экран:
// текст и подсказки элементов, кроме того, что написал человек (заметки, имена, расшифровки).

import table from "./i18n.json";

type Lang = "ru" | "en" | "sr";
const dict = table as Record<string, { en: string; sr: string }>;
let lang: Lang = "ru";
const USER = ".note-text, .note-title, .transcript, .task-title, .task-note, [data-note], [data-folder], [data-file], .card, .note-row, .tab, .result-head, .snippet, .device-code, input, textarea";

export function languageFor(pref: string | undefined): Lang {
  if (pref === "ru" || pref === "en" || pref === "sr") return pref;
  const sys = navigator.language.toLowerCase();
  return sys.startsWith("ru") ? "ru" : sys.startsWith("sr") ? "sr" : sys.startsWith("en") ? "en" : "ru";
}

const tr = (s: string) => {
  const key = s.trim();
  const hit = key && dict[key];
  return hit ? s.replace(key, hit[lang as "en" | "sr"]) : s;
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
  observer = new MutationObserver((list) => list.forEach((m) => m.addedNodes.forEach(translate)));
  observer.observe(document.body, { childList: true, subtree: true });
}
