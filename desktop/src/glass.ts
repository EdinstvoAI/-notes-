// Жидкое стекло капсул шапки (Liquid Glass): WebGL-линза поверх планок. Капсула берёт картинку шапки
// под собой как текстуру и преломляет её: у края сильнее (стекло «загибается»), в середине почти ровно,
// плюс лёгкая матовость, хроматика на кромке и блик сверху. CSS `backdrop-filter` так не умеет.
// На маке под окном потом встанет системный Liquid Glass (tauri-plugin-liquid-glass), это — внутри окна.

const FRAG = `
precision highp float;
uniform sampler2D tex; uniform vec2 size, texSize, origin; uniform float radius, dpr, dark;
float sdBox(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
vec3 pick(vec2 px) { return texture2D(tex, clamp(px / texSize, 0.0, 1.0)).rgb; }
void main() {
  vec2 p = vec2(gl_FragCoord.x, size.y - gl_FragCoord.y);          // пиксели капсулы, y вниз
  vec2 c = p - size * 0.5;
  float r = radius * dpr;
  float d = sdBox(c, size * 0.5, r);                                 // <0 внутри
  if (d > 0.0) { gl_FragColor = vec4(0.0); return; }
  // Нормаль к краю и сила изгиба: в полосе 14 pt у кромки линза тянет картинку внутрь.
  vec2 e = vec2(1.0, 0.0);
  vec2 n = normalize(vec2(sdBox(c + e, size * 0.5, r) - sdBox(c - e, size * 0.5, r), sdBox(c + e.yx, size * 0.5, r) - sdBox(c - e.yx, size * 0.5, r)) + 1e-5);
  float band = 14.0 * dpr;
  float bend = pow(clamp(1.0 + d / band, 0.0, 1.0), 2.5) * 18.0 * dpr;
  vec2 at = origin * dpr + p - n * bend;
  // Матовость — 9 отсчётов вокруг; хроматика — красный и синий чуть врозь у кромки.
  vec3 col = vec3(0.0);
  for (int i = -1; i <= 1; i++) for (int j = -1; j <= 1; j++) col += pick(at + vec2(float(i), float(j)) * 2.5 * dpr);
  col /= 9.0;
  float ca = bend * 0.08;
  col.r = mix(col.r, pick(at - n * ca).r, 0.6);
  col.b = mix(col.b, pick(at + n * ca).b, 0.6);
  // Подсветка стекла (лист #F9F9F9 20 % как на маке), блик по верхней кромке, тень внизу.
  vec3 sheet = dark > 0.5 ? vec3(0.12) : vec3(0.976);
  col = mix(col, sheet, 0.28);
  float rim = smoothstep(-2.0 * dpr, 0.0, d);
  float top = smoothstep(size.y * 0.55, 0.0, p.y) * (1.0 - smoothstep(0.0, band * 1.2, -d));
  col += vec3(1.0) * (rim * (dark > 0.5 ? 0.18 : 0.55) + top * (dark > 0.5 ? 0.06 : 0.22));
  col *= 1.0 - smoothstep(size.y * 0.6, size.y, p.y) * 0.04;
  float a = 1.0 - smoothstep(-1.0, 0.0, d);
  gl_FragColor = vec4(col * a, a);
}`;

interface Lens { canvas: HTMLCanvasElement; gl: WebGLRenderingContext; prog: WebGLProgram; tex: WebGLTexture; host: HTMLElement }
const lenses: Lens[] = [];

function make(host: HTMLElement): Lens | null {
  const canvas = document.createElement("canvas");
  canvas.className = "glass-lens";
  const gl = canvas.getContext("webgl", { premultipliedAlpha: true, alpha: true });
  if (!gl) return null;
  const sh = (type: number, src: string) => { const s = gl.createShader(type)!; gl.shaderSource(s, src); gl.compileShader(s); return s; };
  const prog = gl.createProgram()!;
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, "attribute vec2 a; void main(){ gl_Position = vec4(a, 0.0, 1.0); }"));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FRAG));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null;
  gl.useProgram(prog);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, "a");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  const tex = gl.createTexture()!;
  gl.bindTexture(gl.TEXTURE_2D, tex);
  for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
  host.prepend(canvas);
  host.classList.add("liquid");
  return { canvas, gl, prog, tex, host };
}

/** Перерисовать стекло капсул по картинке шапки `source` (canvas планок). */
export function paintGlass(hosts: HTMLElement[], source: HTMLCanvasElement, dark: boolean) {
  if (!lenses.length) hosts.forEach((h) => { const l = make(h); if (l) lenses.push(l); });
  const dpr = devicePixelRatio || 1;
  const base = source.getBoundingClientRect();
  for (const l of lenses) {
    const { gl, prog, canvas, host } = l;
    if (!canvas.isConnected) host.prepend(canvas);
    const r = host.getBoundingClientRect();
    canvas.width = Math.round(r.width * dpr); canvas.height = Math.round(r.height * dpr);
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.bindTexture(gl.TEXTURE_2D, l.tex);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
    const u = (n: string) => gl.getUniformLocation(prog, n);
    gl.uniform2f(u("size"), canvas.width, canvas.height);
    gl.uniform2f(u("texSize"), source.width, source.height);
    gl.uniform2f(u("origin"), r.left - base.left, r.top - base.top);
    gl.uniform1f(u("radius"), 18); gl.uniform1f(u("dpr"), dpr); gl.uniform1f(u("dark"), dark ? 1 : 0);
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }
}
