// Шейдеры мака (Metal) на WebGL — одна функция на весь прямоугольник canvas. Формулы перенесены из
// `<notes>/Dictation/Orb.metal` и `<notes>/Video/DropZone.metal` строка в строку, чтобы вид совпал.

export type Uniforms = Record<string, number | number[]>;

export function shader(canvas: HTMLCanvasElement, fragment: string): ((u: Uniforms) => void) | null {
  const gl = canvas.getContext("webgl", { premultipliedAlpha: true, alpha: true, antialias: false });
  if (!gl) return null;
  const compile = (type: number, src: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) console.error(gl.getShaderInfoLog(s));
    return s;
  };
  const prog = gl.createProgram()!;
  gl.attachShader(prog, compile(gl.VERTEX_SHADER, "attribute vec2 a; void main(){ gl_Position = vec4(a, 0.0, 1.0); }"));
  gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, "precision highp float;\n" + fragment));
  gl.linkProgram(prog);
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, "a");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  return (u) => {
    const r = devicePixelRatio || 1;
    const w = Math.round(canvas.clientWidth * r), h = Math.round(canvas.clientHeight * r);
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    gl.viewport(0, 0, w, h);
    gl.uniform2f(gl.getUniformLocation(prog, "size"), w, h);
    for (const [name, v] of Object.entries(u)) {
      const l = gl.getUniformLocation(prog, name);
      if (typeof v === "number") gl.uniform1f(l, v); else if (v.length === 4) gl.uniform4fv(l, v);
    }
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };
}

// Позиция как в Metal: начало сверху слева, y вниз.
const POS = "vec2 position = vec2(gl_FragCoord.x, size.y - gl_FragCoord.y);";

export const ORB = `
uniform vec2 size; uniform float t, dark, canvas, think, done; uniform vec4 bands;
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) { vec2 i = floor(p), f = fract(p); vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y); }
float strip(vec2 p, float k, float level, float i) {
  float r2 = dot(p, p);
  vec2 q = p * (1.0 + k * (1.0 - sqrt(max(0.0, 1.0 - r2))));
  float wave = (0.025 + level * 0.3) * sin(q.x * (2.2 + i * 0.7) + t * (1.2 + i * 0.45) + i * 1.7);
  float lineD = q.y - (i - 1.5) * 0.07 - wave;
  float lineW = 0.022 + level * 0.06;
  float lineB = (0.55 + level * 0.9) * smoothstep(1.15, 0.2, abs(q.x));
  float ang = atan(q.y, q.x);
  float dir = mod(i, 2.0) < 0.5 ? 1.0 : -1.0;
  float radius = 0.36 + i * 0.075 + 0.03 * sin(t * 3.2 + i * 0.9);
  float ringD = length(q) - radius - 0.025 * sin(ang * (3.0 + i) + dir * t * (1.6 + i * 0.4));
  float ringB = 0.45 + 0.75 * pow(0.5 + 0.5 * sin(ang * 2.0 - dir * t * 2.4 + i), 2.0);
  float d = mix(lineD, ringD, think);
  float w = mix(lineW, 0.026, think);
  return mix(lineB, ringB, think) * exp(-d * d / (w * w));
}
void main() {
  ${POS}
  vec2 p = (position - size * 0.5) / (size.y * 0.5 / canvas);
  float fill = clamp(done / 0.3, 0.0, 1.0);
  float vapor = clamp((done - 0.35) / 0.65, 0.0, 1.0);
  vec3 green = vec3(0.2, 0.78, 0.38);
  vec2 flow = vec2(0.0, t * 1.6);
  float n = 0.65 * noise(p * vec2(6.0, 3.0) + flow) + 0.35 * noise(p * vec2(14.0, 6.0) + flow * 1.8);
  float r = length(p);
  float voice = max(max(bands.x, bands.y), max(bands.z, bands.w));
  if (r > 1.0) {
    float halo = exp(-(r - 1.0) * 5.0) * (0.10 + voice * 0.15) * (1.0 - vapor * vapor);
    vec3 c = mix(dark > 0.5 ? vec3(1.0) : vec3(0.0), green, fill);
    gl_FragColor = vec4(c * halo, halo); return;
  }
  float y = p.y;
  vec3 top = dark > 0.5 ? vec3(0.02) : vec3(0.18, 0.18, 0.2);
  vec3 bottom = dark > 0.5 ? vec3(0.16, 0.16, 0.18) : vec3(0.78, 0.79, 0.82);
  vec3 body = mix(top, bottom, smoothstep(-0.35, 0.55, y));
  float alpha = mix(dark > 0.5 ? 0.55 : 0.75, 0.95, smoothstep(0.6, 1.0, r));
  vec3 colors0 = vec3(0.898, 0.302, 0.2), colors1 = vec3(0.96, 0.55, 0.28), colors2 = vec3(0.93, 0.36, 0.5), colors3 = vec3(0.62, 0.45, 0.95);
  vec3 light = vec3(0.0);
  for (int i = 0; i < 4; i++) {
    float fi = float(i);
    float b = i == 0 ? bands.x : i == 1 ? bands.y : i == 2 ? bands.z : bands.w;
    vec3 col = i == 0 ? colors0 : i == 1 ? colors1 : i == 2 ? colors2 : colors3;
    vec3 s3 = vec3(strip(p, 0.45, b, fi), strip(p, 0.62, b, fi), strip(p, 0.85, b, fi));
    float lum = dot(s3, vec3(0.3, 0.5, 0.2));
    light += mix(s3, col * lum * 1.9 + lum * 0.2, 0.75);
  }
  float fresnel = pow(smoothstep(0.75, 1.0, r), 3.0);
  float rim = fresnel * (dark > 0.5 ? 0.45 : 0.6);
  float glint = exp(-pow((y - 0.8) * 4.0, 2.0)) * smoothstep(0.6, 0.95, r) * 0.25;
  vec3 c = body + light * (1.1 + voice * 0.8) + rim + glint;
  float a = min(1.0, alpha + max(light.r, light.b) * 0.5 + rim);
  float level = mix(1.1, -1.1, fill) + 0.04 * sin(p.x * 7.0 + t * 6.0);
  float wet = smoothstep(-0.03, 0.03, y - level);
  c = mix(c, green * (0.85 + 0.3 * smoothstep(-1.0, 1.0, y)) + rim * 0.6 + glint, wet * 0.95);
  a = mix(a, 1.0, wet);
  float front = mix(-1.25, 1.25, vapor);
  float keep = smoothstep(front - 0.12, front + 0.12, y + (n - 0.5) * 0.6);
  float steam = (1.0 - keep) * smoothstep(0.45, 0.85, n) * (1.0 - vapor) * fill;
  c = mix(c, vec3(0.75, 1.0, 0.82), (1.0 - keep) * fill);
  c += vec3(0.6, 1.0, 0.7) * keep * (1.0 - keep) * 2.5 * fill;
  a = a * keep + steam * 0.55;
  a *= smoothstep(1.0, 0.985, r);
  gl_FragColor = vec4(c * a, a);
}`;

export const DROPZONE = `
uniform vec2 size; uniform float t, on, dark, scale;
vec3 footage(vec2 p) {
  float speed = 0.25 + on * 0.35;
  vec2 a = vec2(0.25 + 0.2 * sin(t * speed * 0.9), 0.5 + 0.35 * cos(t * speed * 0.7));
  vec2 b = vec2(0.7 + 0.2 * cos(t * speed * 0.6 + 1.3), 0.45 + 0.4 * sin(t * speed * 0.8 + 0.4));
  vec2 c = vec2(0.5 + 0.35 * sin(t * speed * 0.5 + 2.1), 0.6 + 0.3 * cos(t * speed * 1.1 + 2.7));
  float ga = exp(-dot(p - a, p - a) * 9.0) * 0.9, gb = exp(-dot(p - b, p - b) * 11.0) * 0.55, gc = exp(-dot(p - c, p - c) * 14.0) * 0.35;
  vec3 coral = vec3(0.898, 0.302, 0.2), amber = vec3(0.95, 0.62, 0.25), lilac = vec3(0.55, 0.42, 0.9);
  vec3 night = (vec3(0.03, 0.03, 0.035) + coral * ga + amber * gb + lilac * gc) * (0.55 + on * 0.6);
  vec3 day = vec3(0.96, 0.96, 0.965);
  float k = 0.35 + on * 0.3;
  day = mix(day, coral, clamp(ga * k, 0.0, 1.0));
  day = mix(day, amber, clamp(gb * k, 0.0, 1.0));
  day = mix(day, lilac, clamp(gc * k, 0.0, 1.0));
  return mix(day, night, dark);
}
void main() {
  ${POS}
  float slat = 20.0 * scale; // планка 20 pt — в пикселях экрана
  float local = mod(position.x, slat);
  float center = position.x - local + slat * 0.5;
  float x = center + (local - slat * 0.5) / 1.8;
  vec2 uv = vec2(x, position.y) / size;
  vec3 col = vec3(0.0);
  for (int i = -2; i <= 2; i++) col += footage(uv + vec2(0.0, float(i) * 0.02));
  col /= 5.0;
  float edge = local / slat;
  col += mix(0.03, 0.06, dark) * smoothstep(0.12, 0.0, edge);
  col *= 1.0 - mix(0.08, 0.25, dark) * smoothstep(0.7, 1.0, edge);
  col *= mix(1.0, 0.85, dark);
  gl_FragColor = vec4(col, 1.0);
}`;
