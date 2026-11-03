// Формулировки под ОС: на маке — Finder, ⌘, Touch ID и Связка ключей, на Windows — Проводник, Ctrl, Windows Hello.
// По userAgent, а не по классу body: шапка рисуется раньше, чем приложение узнаёт ОС у Tauri.
export const isMac = /Mac/.test(navigator.userAgent);
export const os = (windows: string, mac: string) => (isMac ? mac : windows);
export const hello = os("windows hello", "touch id");
