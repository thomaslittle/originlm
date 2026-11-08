/* The Origin Stone: one faceted obsidian monolith, rendered by intersecting a ray with a
   convex polyhedron (28 half-spaces) in a single fragment shader pass. No scene graph,
   no mesh, no raymarching loop: one pass over 28 planes per pixel, plus eight orbiting
   point lights, a beam and a dial drawn analytically in screen space. */

export const CAMZ = 4.6;
export const FOCAL = 1.9;
/** Stone half-height in object units (tip of the upper facets). */
export const STONE_HALF = 1.33;
export const LIGHTS = 8;

export type Uniforms = {
  shift: [number, number, number];
  scale: number;
  rotY: number;
  tilt: number;
  mouse: [number, number];
  lights: Float32Array; // LIGHTS * vec4 (xyz world, w intensity)
  facet: number;
  beam: number;
  beamA: [number, number];
  beamB: [number, number];
  dial: number;
  dialAmt: number;
  dim: number;
  time: number;
};

const VERT = `attribute vec2 aPos; void main(){ gl_Position = vec4(aPos, 0.0, 1.0); }`;

const FRAG = `
precision highp float;
uniform vec2 uRes;
uniform float uTime;
uniform vec3 uShift;
uniform float uScale;
uniform vec2 uRot;
uniform vec2 uMouse;
uniform vec4 uL[${LIGHTS}];
uniform float uFacet;
uniform float uBeam;
uniform vec2 uBeamA;
uniform vec2 uBeamB;
uniform float uDial;
uniform float uDialAmt;
uniform float uDim;
const float CAMZ = ${CAMZ.toFixed(2)};
const float FOCAL = ${FOCAL.toFixed(2)};
const float TAU = 6.2831853;

float hash(vec2 p){ return fract(sin(dot(p, vec2(41.3, 289.1))) * 43758.5453); }

void planeAt(int i, out vec3 n, out float d){
  float fi = float(i);
  float g = floor(fi / 7.0);
  float k = fi - g * 7.0;
  float a = k * TAU / 7.0;
  if (g < 0.5) { n = vec3(cos(a), 0.0, sin(a)); d = 0.52; }
  else if (g < 1.5) { a += TAU / 14.0; n = normalize(vec3(cos(a), 1.2, sin(a))); d = 0.657; }
  else if (g < 2.5) { n = normalize(vec3(cos(a), -1.2, sin(a))); d = 0.657; }
  else { a += TAU / 14.0; n = normalize(vec3(cos(a), 0.22, sin(a))); d = 0.545; }
}

vec3 env(vec3 r, vec3 L){
  // A black studio: one cold key, a long strip light, a faint rim. Chrome reads as
  // hard reflections of these against nothing.
  float key = pow(max(dot(r, L), 0.0), 48.0);
  float soft = pow(max(dot(r, L), 0.0), 6.0);
  vec3 sd = normalize(vec3(-0.9, 0.15, 0.4));
  float strip = smoothstep(0.86, 0.95, dot(normalize(vec3(r.x, 0.0, r.z)), normalize(vec3(sd.x, 0.0, sd.z)))) * smoothstep(-0.35, 0.1, r.y) * (1.0 - smoothstep(0.55, 0.9, r.y));
  float rim = smoothstep(0.93, 0.99, dot(r, normalize(vec3(0.95, 0.2, -0.3))));
  vec3 c = vec3(0.0012, 0.0014, 0.0022);
  c += vec3(0.70, 0.82, 1.0) * key * 4.0;
  c += vec3(0.05, 0.07, 0.11) * soft * 0.35;
  c += vec3(0.55, 0.66, 0.85) * strip * 0.7;
  c += vec3(0.30, 0.40, 0.62) * rim * 0.6;
  return c;
}

void main(){
  vec2 uv = (gl_FragCoord.xy - 0.5 * uRes) / uRes.y;
  vec3 ro = vec3(0.0, 0.0, CAMZ);
  vec3 rd = normalize(vec3(uv, -FOCAL));

  float cy = cos(uRot.x), sy = sin(uRot.x), cx = cos(uRot.y), sx = sin(uRot.y);
  mat3 ry = mat3(cy, 0.0, -sy, 0.0, 1.0, 0.0, sy, 0.0, cy);
  mat3 rx = mat3(1.0, 0.0, 0.0, 0.0, cx, sx, 0.0, -sx, cx);
  mat3 M = rx * ry;
  vec3 roo = M * (ro - uShift) / uScale;
  vec3 rdo = M * rd / uScale;

  float tN = -1e9, tN2 = -1e9, tF = 1e9;
  vec3 nN = vec3(0.0, 0.0, 1.0);
  float fid = -1.0;
  for (int i = 0; i < 28; i++) {
    vec3 n; float d;
    planeAt(i, n, d);
    float vn = dot(rdo, n);
    float pn = dot(roo, n) - d;
    float t = -pn / vn;
    if (vn < 0.0) {
      if (t > tN) { tN2 = tN; tN = t; nN = n; fid = float(i); }
      else if (t > tN2) { tN2 = t; }
    } else {
      tF = min(tF, t);
    }
  }
  bool hit = tN < tF && tN > 0.0;

  vec2 sc = uShift.xy * FOCAL / (CAMZ - uShift.z);
  vec3 L = normalize(vec3(-0.65 + uMouse.x * 0.9, 0.85 + uMouse.y * 0.6, 0.6));

  // Ground: deep black, a cold haze that follows the stone.
  float rr = length(uv - sc);
  vec3 col = vec3(0.00012, 0.00012, 0.00016) + vec3(0.0012, 0.0017, 0.0034) * exp(-rr * 3.0 / max(uScale, 0.2)) * (1.0 - uDim * 0.8);

  if (hit) {
    vec3 p = ro + rd * tN;
    vec3 n = nN * M; // object -> world (M is a rotation)
    vec3 r = reflect(rd, n);
    float ndv = max(dot(-rd, n), 0.0);
    float fres = 0.04 + 0.96 * pow(1.0 - ndv, 5.0);
    vec3 base = vec3(0.002, 0.0025, 0.004) + vec3(0.016, 0.020, 0.030) * max(dot(n, L), 0.0);
    vec3 refl = env(r, L);
    vec3 c = base + refl * (0.22 + 0.78 * fres);

    // Orbit lights glint in the facets.
    for (int i = 0; i < ${LIGHTS}; i++) {
      vec3 Ld = uL[i].xyz - p;
      float s = pow(max(dot(r, normalize(Ld)), 0.0), 90.0);
      c += vec3(0.75, 0.86, 1.0) * s * uL[i].w * 1.6;
      c += vec3(0.03, 0.04, 0.06) * max(dot(n, normalize(Ld)), 0.0) * uL[i].w / (1.0 + dot(Ld, Ld));
    }

    // Chrome edges where two facets meet.
    float edge = 1.0 - smoothstep(0.0, 0.018 * uScale, tN - tN2);
    c += vec3(0.42, 0.52, 0.72) * edge * (0.18 + 0.5 * fres + 0.3 * max(dot(n, L), 0.0));

    // Grant: the facet facing you opens onto the core; the rest of the stone darkens.
    c *= 1.0 - 0.45 * uFacet;
    if (fid < 6.5 || fid > 20.5) {
      float facing = smoothstep(0.86, 0.965, n.z);
      vec3 q = M * (p - uShift) / uScale;
      vec3 side = vec3(-nN.z, 0.0, nN.x);
      float u = dot(q, side);
      float seam = exp(-abs(u) * 28.0) * (1.0 - smoothstep(0.55, 0.92, abs(q.y)));
      float core = (1.0 - smoothstep(0.0, 0.95, abs(q.y))) * (1.0 - smoothstep(0.0, 0.32, abs(u)));
      float pulse = 0.85 + 0.15 * sin(uTime * 2.2);
      c += uFacet * facing * (vec3(0.30, 0.48, 0.95) * core * 0.55 + vec3(0.85, 0.93, 1.0) * seam * 2.6) * pulse;
      c += uFacet * facing * edge * vec3(0.6, 0.78, 1.0) * 1.4;
    }
    col = c * (1.0 - uDim * 0.72);
  }

  // Orbiting points of light: one per detected client.
  for (int i = 0; i < ${LIGHTS}; i++) {
    vec3 w = uL[i].xyz - ro;
    float tl = dot(w, rd);
    float dist = length(w - rd * tl);
    float occl = (hit && tN < tl) ? 0.0 : 1.0;
    float g = uL[i].w * occl;
    col += vec3(0.80, 0.88, 1.0) * g * (0.00030 / (dist * dist + 0.00008)) * exp(-dist * 4.0);
    col += vec3(0.22, 0.30, 0.50) * g * 0.0035 * exp(-dist * 11.0);
  }

  // Call: a beam from the stone to your code.
  if (uBeam > 0.001) {
    vec2 ab = uBeamB - uBeamA;
    float h = clamp(dot(uv - uBeamA, ab) / dot(ab, ab), 0.0, 1.0);
    float d = length(uv - uBeamA - ab * h);
    float travel = 0.55 + 0.45 * smoothstep(0.65, 1.0, sin(h * 34.0 - uTime * 7.0));
    float reach = smoothstep(uBeam * 1.05, uBeam * 1.05 - 0.05, h);
    col += vec3(0.55, 0.72, 1.0) * reach * travel * (0.0009 / (d * d + 0.00012)) * 0.35;
    col += vec3(0.85, 0.93, 1.0) * reach * exp(-d * 900.0) * 0.6;
  }

  // Control: the usage dial, light read as a reading.
  if (uDialAmt > 0.001) {
    vec2 q = uv - sc;
    float r0 = ${STONE_HALF.toFixed(2)} * uScale * FOCAL / CAMZ * 1.1;
    float ang = fract(0.25 - atan(q.y, q.x) / TAU);
    float ring = exp(-pow((length(q) - r0) / 0.0022, 2.0));
    float ticks = step(0.55, fract(ang * 60.0)) * exp(-pow((length(q) - r0 - 0.018) / 0.004, 2.0));
    float lit = step(ang, uDial);
    col += uDialAmt * (vec3(0.70, 0.84, 1.0) * ring * (0.12 + 1.6 * lit) + vec3(0.25, 0.32, 0.45) * ticks * 0.5);
  }

  col = 1.0 - exp(-col * 1.25);
  col = pow(col, vec3(1.0 / 2.2));
  col += (hash(gl_FragCoord.xy + fract(uTime)) - 0.5) / 255.0;
  gl_FragColor = vec4(col, 1.0);
}`;

export type Stone = {
  draw: (u: Uniforms) => void;
  resize: (width: number, height: number, dpr: number) => void;
  destroy: () => void;
  canvas: HTMLCanvasElement;
};

export function createStone(canvas: HTMLCanvasElement, preserve = false, disposable = false): Stone | null {
  let gl: WebGLRenderingContext | null = null;
  try {
    gl = canvas.getContext("webgl", { antialias: false, alpha: false, preserveDrawingBuffer: preserve, powerPreference: "high-performance" }) as WebGLRenderingContext | null;
  } catch { gl = null; }
  if (!gl) return null;
  const g = gl;
  const compile = (type: number, src: string) => {
    const s = g.createShader(type)!;
    g.shaderSource(s, src);
    g.compileShader(s);
    if (!g.getShaderParameter(s, g.COMPILE_STATUS)) { console.warn(g.getShaderInfoLog(s)); return null; }
    return s;
  };
  const vs = compile(g.VERTEX_SHADER, VERT);
  const fs = compile(g.FRAGMENT_SHADER, FRAG);
  if (!vs || !fs) return null;
  const prog = g.createProgram()!;
  g.attachShader(prog, vs);
  g.attachShader(prog, fs);
  g.linkProgram(prog);
  if (!g.getProgramParameter(prog, g.LINK_STATUS)) { console.warn(g.getProgramInfoLog(prog)); return null; }
  g.useProgram(prog);
  const buf = g.createBuffer();
  g.bindBuffer(g.ARRAY_BUFFER, buf);
  g.bufferData(g.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), g.STATIC_DRAW);
  const loc = g.getAttribLocation(prog, "aPos");
  g.enableVertexAttribArray(loc);
  g.vertexAttribPointer(loc, 2, g.FLOAT, false, 0, 0);
  const U = (name: string) => g.getUniformLocation(prog, name);
  const u = {
    res: U("uRes"), time: U("uTime"), shift: U("uShift"), scale: U("uScale"), rot: U("uRot"), mouse: U("uMouse"),
    lights: U("uL[0]") ?? U("uL"), facet: U("uFacet"), beam: U("uBeam"), beamA: U("uBeamA"), beamB: U("uBeamB"),
    dial: U("uDial"), dialAmt: U("uDialAmt"), dim: U("uDim"),
  };

  return {
    canvas,
    resize(width, height, dpr) {
      canvas.width = Math.max(1, Math.round(width * dpr));
      canvas.height = Math.max(1, Math.round(height * dpr));
      g.viewport(0, 0, canvas.width, canvas.height);
    },
    draw(v) {
      g.uniform2f(u.res, canvas.width, canvas.height);
      g.uniform1f(u.time, v.time);
      g.uniform3f(u.shift, ...v.shift);
      g.uniform1f(u.scale, v.scale);
      g.uniform2f(u.rot, v.rotY, v.tilt);
      g.uniform2f(u.mouse, ...v.mouse);
      g.uniform4fv(u.lights, v.lights);
      g.uniform1f(u.facet, v.facet);
      g.uniform1f(u.beam, v.beam);
      g.uniform2f(u.beamA, ...v.beamA);
      g.uniform2f(u.beamB, ...v.beamB);
      g.uniform1f(u.dial, v.dial);
      g.uniform1f(u.dialAmt, v.dialAmt);
      g.uniform1f(u.dim, v.dim);
      g.drawArrays(g.TRIANGLES, 0, 3);
    },
    destroy() {
      g.deleteProgram(prog);
      g.deleteBuffer(buf);
      // A live canvas may be re-attached (React StrictMode re-runs effects): only lose
      // the context of a throwaway canvas.
      if (disposable) g.getExtension("WEBGL_lose_context")?.loseContext();
    },
  };
}

/* ---------- Pose maths shared by the live canvas, the stills and the DOM labels ---------- */

/** A beat's framing, in screen terms: centre as a fraction of the viewport, stone height
 *  as a fraction of viewport height. */
export type Pose = {
  cx: number; cy: number; size: number;
  lights: number; labels: number; facet: number; beam: number; dial: number; dialAmt: number; dim: number; spin: number;
};

export const POSE_KEYS: (keyof Pose)[] = ["cx", "cy", "size", "lights", "labels", "facet", "beam", "dial", "dialAmt", "dim", "spin"];

export function lerpPose(a: Pose, b: Pose, t: number): Pose {
  const out = { ...a };
  for (const k of POSE_KEYS) out[k] = a[k] + (b[k] - a[k]) * t;
  return out;
}

/** World-space centre and scale for a pose at a given aspect ratio. */
export function frame(pose: Pose, aspect: number): { shift: [number, number, number]; scale: number } {
  const uvx = (pose.cx - 0.5) * aspect;
  const uvy = (0.5 - pose.cy);
  return {
    shift: [uvx * CAMZ / FOCAL, uvy * CAMZ / FOCAL, 0],
    scale: Math.max(0.05, pose.size / (2 * STONE_HALF * FOCAL / CAMZ)),
  };
}

/** Orbit positions of the lights. Lights appear one by one as `lights` rises. */
export function orbit(out: Float32Array, time: number, pose: Pose, shift: [number, number, number], scale: number, mouseWorld: [number, number] | null): Float32Array {
  const tilt = 0.32;
  for (let i = 0; i < LIGHTS; i++) {
    const a = time * 0.16 + (i * Math.PI * 2) / LIGHTS;
    const R = scale * (1.5 + 0.12 * Math.sin(i * 1.7));
    let x = R * Math.cos(a);
    const flat = R * Math.sin(a);
    let y = flat * Math.sin(tilt) + scale * 0.32 * Math.sin(a * 2 + i);
    const z = flat * Math.cos(tilt);
    x += shift[0]; y += shift[1];
    if (mouseWorld) {
      const dx = mouseWorld[0] - x, dy = mouseWorld[1] - y;
      const pull = 0.35 * scale / (1 + (dx * dx + dy * dy) * 3);
      x += dx * pull; y += dy * pull;
    }
    const appear = Math.min(1, Math.max(0, pose.lights * LIGHTS * 1.15 - i));
    out[i * 4] = x; out[i * 4 + 1] = y; out[i * 4 + 2] = z + shift[2]; out[i * 4 + 3] = appear;
  }
  return out;
}

/** World point -> CSS pixel. */
export function project(x: number, y: number, z: number, width: number, height: number): [number, number] {
  const k = FOCAL / (CAMZ - z);
  return [width / 2 + x * k * height, height / 2 - y * k * height];
}

/** CSS pixel -> uv (shader space). */
export function toUv(px: number, py: number, width: number, height: number): [number, number] {
  return [(px - width / 2) / height, (height / 2 - py) / height];
}

export function uniformsFor(pose: Pose, width: number, height: number, time: number, rotY: number, lights = new Float32Array(LIGHTS * 4)): Uniforms {
  const { shift, scale } = frame(pose, width / height);
  orbit(lights, time, pose, shift, scale, null);
  const sc: [number, number] = [shift[0] * FOCAL / CAMZ, shift[1] * FOCAL / CAMZ];
  return {
    shift, scale, rotY, tilt: 0.12, mouse: [0, 0], lights,
    facet: pose.facet, beam: pose.beam, beamA: sc, beamB: [sc[0] + 0.9, sc[1]],
    dial: pose.dial, dialAmt: pose.dialAmt, dim: pose.dim, time,
  };
}

/** Render a set of poses to image URLs once, for reduced motion and ?still. */
export function renderStills(jobs: { pose: Pose; width: number; height: number; time?: number; rot?: number; beamTo?: [number, number] }[]): string[] | null {
  if (typeof document === "undefined") return null;
  const canvas = document.createElement("canvas");
  const stone = createStone(canvas, true, true);
  if (!stone) return null;
  const out: string[] = [];
  for (const job of jobs) {
    stone.resize(job.width, job.height, 1);
    const u = uniformsFor(job.pose, job.width, job.height, job.time ?? 11.2, job.rot ?? 0.38);
    if (job.beamTo) u.beamB = job.beamTo;
    stone.draw(u);
    out.push(canvas.toDataURL("image/png"));
  }
  stone.destroy();
  return out;
}
