// Смена страницы внутри раздела (настройки, mcp → скиллы, вид в паролях) — проявлением из размытия
// за 0,45 с, как .blurReplace на маке (SectionContent.contentID), а не рывком.
export function blurIn(el: HTMLElement) {
  el.classList.remove("blur-in");
  void el.offsetWidth;
  el.classList.add("blur-in");
}

// Вторая половина .blurReplace: копия старого уходит в размытие, пока новое проявляется. Снять копию надо
// до замены, поставить — после: `const out = leaving(el); …замена…; out(родитель)`.
export function leaving(el: HTMLElement) {
  const ghost = el.cloneNode(true) as HTMLElement;
  const box = { left: el.offsetLeft, top: el.offsetTop, width: el.offsetWidth, height: el.offsetHeight };
  const scrolled = [el, ...el.querySelectorAll<HTMLElement>("*")].map((n, i) => [i, n.scrollTop] as const).filter(([, y]) => y > 0);
  return (parent: HTMLElement) => {
    ghost.removeAttribute("id");
    ghost.classList.remove("blur-in");
    ghost.classList.add("blur-out");
    Object.assign(ghost.style, { left: `${box.left}px`, top: `${box.top}px`, width: `${box.width}px`, height: `${box.height}px`, bottom: "auto" });
    parent.append(ghost);
    const all = [ghost, ...ghost.querySelectorAll<HTMLElement>("*")];
    for (const [i, y] of scrolled) all[i].scrollTop = y;
    setTimeout(() => ghost.remove(), 450);
  };
}
