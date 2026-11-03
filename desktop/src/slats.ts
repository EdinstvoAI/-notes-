// Полоса стеклянных планок поверх шахматки — шапка окна. Порт `GlassSlats.swift`:
// каждая планка увеличивает шахматку под собой в 1,8 раза и матует её (blur 3), по левому краю — блик.
// Рисуем один раз на ширину окна, а не на каждый кадр.

const CELLS_ACROSS = 16;
const MAGNIFICATION = 1.8;
const FROST = 3;
const SLAT = 20;

function palette(dark: boolean) {
  return dark
    ? { dark: "rgb(3,3,3)", light: "rgb(13,13,13)", glint: "rgba(255,255,255,0.12)" }
    : { dark: "rgb(230,230,230)", light: "rgb(246,246,246)", glint: "rgba(255,255,255,0.7)" };
}

export function drawSlats(canvas: HTMLCanvasElement, width: number, height: number, dark: boolean, slat = SLAT) {
  const SLATW = Math.max(1, slat);
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * ratio);
  canvas.height = Math.round(height * ratio);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  const c = palette(dark);

  const cell = width / CELLS_ACROSS;
  const rows = Math.ceil(height / cell);
  const count = Math.floor(Math.ceil(width / SLATW) / 2) * 2 + 3;
  let left = (width - count * SLATW) / 2;

  ctx.fillStyle = c.light;
  ctx.fillRect(0, 0, width, height);

  for (let i = 0; i < count; i++) {
    const center = left + SLATW / 2;
    ctx.save();
    ctx.beginPath();
    ctx.rect(left, 0, SLATW, height);
    ctx.clip();
    ctx.filter = `blur(${FROST}px)`;
    ctx.fillStyle = c.light;
    ctx.fillRect(left - FROST * 2, -FROST * 2, SLATW + FROST * 4, height + FROST * 4);
    ctx.fillStyle = c.dark;
    // Какой кусок шахматки видно сквозь планку: середина та же, ширина в 1,8 раза меньше.
    const sampleStart = center - SLATW / (2 * MAGNIFICATION);
    const sampleEnd = center + SLATW / (2 * MAGNIFICATION);
    let sample = sampleStart;
    while (sample < sampleEnd - 0.001) {
      const column = Math.floor(sample / cell);
      const next = Math.min(sampleEnd, (column + 1) * cell);
      const x0 = sample <= sampleStart + 0.001 ? left - FROST * 2 : center + (sample - center) * MAGNIFICATION;
      const x1 = next >= sampleEnd - 0.001 ? left + SLATW + FROST * 2 : center + (next - center) * MAGNIFICATION;
      for (let row = 0; row < rows; row++) {
        if (((column + row) % 2 + 2) % 2 === 0) ctx.fillRect(x0, row * cell, x1 - x0, cell);
      }
      sample = next;
    }
    ctx.restore();
    ctx.fillStyle = c.glint;
    ctx.fillRect(left, 0, 1, height);
    left += SLATW;
  }
}
