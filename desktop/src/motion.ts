// Смена страницы внутри раздела (настройки, mcp → скиллы, вид в паролях) — проявлением из размытия
// за 0,45 с, как .blurReplace на маке (SectionContent.contentID), а не рывком.
export function blurIn(el: HTMLElement) {
  el.classList.remove("blur-in");
  void el.offsetWidth;
  el.classList.add("blur-in");
}
