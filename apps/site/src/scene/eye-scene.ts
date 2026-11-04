/* The eye. One WebGL2 scene for the whole page, lazy-loaded after first paint.

   Passes, back to front, into a half-float buffer:
     field     the dark, lit only by the eye (amber is the only light source)
     dust      motes at depth, sized by their distance from the focal plane
     ribbons   threads node -> runtime core, the machine outline, provider lines
               (screen-space strips; per-channel falloff gives the chromatic glow)
     core      the runtime, behind the eye, seen through its opening
     nodes     the clients the eye finds; out-of-focus nodes grow soft and dim
     eye       the real logo paths rasterised to masks; the opening moves with gaze,
               the lid squashes it for the wake and the blink
     shutter   the six-blade consent aperture
   then a composite: highlight shoulder, edge aberration, vignette, film grain.

   No three.js: everything here is billboards and strips, and three would add ~130 KB
   gzip without drawing any of it. See docs/design/landing-wow-plan.md. */

import geometry from "../../../../brand/logo-geometry.json";
import { anchorSink, sceneState, type Anchor } from "./state";

export type SceneTier = "high" | "low";
export type SceneOptions = {
  tier: SceneTier;
  capture?: boolean;
  onFirstFrame?: () => void;
  onFailure?: (reason: string) => void;
};
export type EyeScene = { dispose: () => void; stats: () => SceneStats };
export type SceneStats = { frames: number; avgMs: number; p95Ms: number; renderScale: number; tier: SceneTier; renderer: string };

/* ---------- palette (sRGB hex -> linear) ---------- */
const lin = (hex: string): [number, number, number] => {
  const n = parseInt(hex.slice(1), 16);
  const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return [c[0], c[1], c[2]];
};
const C = {
  unlit: lin("#120f0f"),
  graphite: lin("#231f20"),
  amber: lin("#eba31a"),
  hot: lin("#f7bd4b"),
  filament: lin("#fff2dc"),
};

/* ---------- small math ---------- */
type V3 = [number, number, number];
const clamp01 = (x: number) => (x < 0 ? 0 : x > 1 ? 1 : x);
const mix = (a: number, b: number, t: number) => a + (b - a) * t;
const mix3 = (a: V3, b: V3, t: number): V3 => [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)];
const smooth = (a: number, b: number, x: number) => { const t = clamp01((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const expoOut = (x: number) => (x >= 1 ? 1 : 1 - Math.pow(2, -10 * clamp01(x)));
const quintOut = (x: number) => 1 - Math.pow(1 - clamp01(x), 5);
const quartInOut = (x: number) => { const t = clamp01(x); return t < 0.5 ? 8 * t ** 4 : 1 - Math.pow(-2 * t + 2, 4) / 2; };
const span = (x: number, a: number, b: number) => clamp01((x - a) / (b - a));

function perspective(fovy: number, aspect: number, near: number, far: number) {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
}
function lookAt(eye: V3, target: V3): Float32Array {
  let zx = eye[0] - target[0], zy = eye[1] - target[1], zz = eye[2] - target[2];
  let l = Math.hypot(zx, zy, zz); zx /= l; zy /= l; zz /= l;
  // x = up(0,1,0) cross z
  let xx = zz, xy = 0, xz = -zx;
  l = Math.hypot(xx, xy, xz); xx /= l; xy /= l; xz /= l;
  const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
  return new Float32Array([
    xx, yx, zx, 0, xy, yy, zy, 0, xz, yz, zz, 0,
    -(xx * eye[0] + xy * eye[1] + xz * eye[2]), -(yx * eye[0] + yy * eye[1] + yz * eye[2]), -(zx * eye[0] + zy * eye[1] + zz * eye[2]), 1,
  ]);
}
function mul(a: Float32Array, b: Float32Array) {
  const o = new Float32Array(16);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
  }
  return o;
}

/* ---------- layout ---------- */
const FOV = (32 * Math.PI) / 180;
const TAN = Math.tan(FOV / 2);
type Framing = { ex: number; ey: number; eyeH: number; ring: number; camZ: number };
type Layout = {
  hero: Framing; story: Framing; pull: Framing; cta: Framing;
  nodes: V3[]; front: V3; machine: [number, number];
};
const WIDE: Layout = {
  hero: { ex: 0.45, ey: 0.2, eyeH: 2.35, ring: 1, camZ: 10 },
  story: { ex: 0.5, ey: 0.42, eyeH: 1.42, ring: 0.66, camZ: 9.4 },
  pull: { ex: 0.12, ey: 0.3, eyeH: 1.5, ring: 0.74, camZ: 21 },
  cta: { ex: 0.47, ey: 0.06, eyeH: 2.0, ring: 0.86, camZ: 10 },
  nodes: [[-2.75, 0.95, 1.1], [-0.85, 2.1, -1.9], [1.85, 1.3, -0.9], [1.4, -1.75, 0.6], [-1.75, -1.35, -2.4]],
  front: [-2.7, 0.25, 3.2],
  machine: [1.42, 1.3],
};
const NARROW: Layout = {
  hero: { ex: -0.04, ey: 0.45, eyeH: 1.0, ring: 0.36, camZ: 10 },
  story: { ex: 0.0, ey: 0.62, eyeH: 0.52, ring: 0.27, camZ: 9.6 },
  pull: { ex: 0.0, ey: 0.2, eyeH: 0.66, ring: 0.31, camZ: 15.5 },
  cta: { ex: 0.0, ey: 0.42, eyeH: 0.92, ring: 0.4, camZ: 10 },
  nodes: [[-2.3, 1.45, 1.0], [0.35, 2.75, -1.8], [2.35, 1.05, -0.6], [1.95, -1.95, 0.8], [-1.95, -1.7, -2.2]],
  front: [-0.2, -3.6, 3.0],
  machine: [1.5, 1.34],
};
const mixFraming = (a: Framing, b: Framing, t: number): Framing => ({
  ex: mix(a.ex, b.ex, t), ey: mix(a.ey, b.ey, t), eyeH: mix(a.eyeH, b.eyeH, t), ring: mix(a.ring, b.ring, t), camZ: mix(a.camZ, b.camZ, t),
});

/* The mark: 205 x 208 units. The opening's centre, from its path bounds. */
const MARK_W = 205, MARK_H = 208, OPEN_CX = 118.6, OPEN_CY = 86.97;
/* The opening wanders inside an ellipse a little wider than the logo's 12 x 10 rule,
   because the hero mark is drawn ~25x larger than the nav logo. */
const GAZE_X = 14, GAZE_Y = 11.5;

/* ---------- shaders ---------- */
const HEAD = "#version 300 es\nprecision highp float;\n";
const NOISE = `
float h21(vec2 p){ p=fract(p*vec2(123.34,456.21)); p+=dot(p,p+45.32); return fract(p.x*p.y); }
float vnoise(vec2 p){ vec2 i=floor(p), f=fract(p); f=f*f*(3.-2.*f);
  return mix(mix(h21(i),h21(i+vec2(1,0)),f.x), mix(h21(i+vec2(0,1)),h21(i+vec2(1,1)),f.x), f.y); }`;
const FULL_VS = `${HEAD}
out vec2 vUv;
void main(){ vec2 p=vec2(gl_VertexID==1?3.:-1., gl_VertexID==2?3.:-1.); vUv=p*.5+.5; gl_Position=vec4(p,0,1); }`;
const FIELD_FS = `${HEAD}${NOISE}
in vec2 vUv; out vec4 o;
uniform vec2 uRes; uniform vec2 uEye; uniform float uEyeR; uniform float uLight; uniform float uTime;
uniform vec3 uUnlit; uniform vec3 uAmber;
void main(){
  vec2 px=vec2(vUv.x,1.-vUv.y)*uRes;
  vec2 q=(px-uEye)/uEyeR; float d2=dot(q,q);
  vec2 s=px/uRes.y;
  float haze=vnoise(s*2.1+vec2(uTime*.011,-uTime*.007))*.62+vnoise(s*5.3-vec2(uTime*.019,0.))*.38;
  float light=uLight*(.085/(1.+d2*1.25)+.011/(1.+d2*.06));
  o=vec4(uUnlit+uAmber*light*(.3+haze*.95),1.);
}`;
const DUST_VS = `${HEAD}
in vec4 aP;
uniform mat4 uVP; uniform float uTime; uniform float uFocus; uniform float uPxW; uniform vec3 uEyeW; uniform float uAmt;
out float vA; out float vB;
void main(){
  float s=aP.w;
  vec3 p=aP.xyz+vec3(sin(uTime*.07+s*6.28)*.09, cos(uTime*.05+s*4.1)*.09 + uTime*.012*(fract(s*3.7)-.3), 0.);
  vec4 c=uVP*vec4(p,1.); gl_Position=c;
  float blur=clamp(abs(c.w-uFocus)*.32,0.,3.2);
  gl_PointSize=clamp((.012+blur*.018)*uPxW/c.w,1.,64.);
  float lit=1./(1.+pow(distance(p,uEyeW)*.42,2.));
  vA=uAmt*lit*(.25+.75*fract(s*7.13))/(1.+blur*blur*1.6); vB=blur;
}`;
const DUST_FS = `${HEAD}
in float vA; in float vB; out vec4 o; uniform vec3 uAmber; uniform vec3 uFil;
void main(){ float r=length(gl_PointCoord-.5)*2.; float a=smoothstep(1.,mix(.2,.0,clamp(vB/2.,0.,1.)),r);
  o=vec4(mix(uFil,uAmber,clamp(vB*.6,0.,1.))*a*vA,0.); }`;
const RIBBON_VS = `${HEAD}
in vec2 aPx; in float aU; in float aSide; in float aHalf; in float aCore; in float aLum;
uniform vec2 uRes;
out float vU; out float vD; out float vCore; out float vLum; out float vHalf;
void main(){ vU=aU; vD=aSide*aHalf; vCore=aCore; vLum=aLum; vHalf=aHalf;
  gl_Position=vec4(aPx.x/uRes.x*2.-1., 1.-aPx.y/uRes.y*2., 0., 1.); }`;
const RIBBON_FS = `${HEAD}
in float vU; in float vD; in float vCore; in float vLum; in float vHalf; out vec4 o;
uniform float uDraw; uniform float uInt; uniform float uTime; uniform float uPulse; uniform float uDash; uniform float uFadeEnd; uniform float uPx;
uniform vec3 uAmber; uniform vec3 uFil;
void main(){
  float d=abs(vD);
  float drawn=smoothstep(uDraw+.006,uDraw-.006,vU);
  if(drawn<=0.) discard;
  float c=vCore;
  float core=exp(-(d*d)/(c*c));
  vec3 s=vec3(1.6,1.,.55)*(c*1.7+2.*uPx);
  float win=1.-smoothstep(.45,1.,d/vHalf);
  vec3 glow=exp(-d/s)*win;
  float tip=exp(-pow((uDraw-vU)*55.,2.))*step(uDraw,.995);
  float pulse=0.;
  for(int i=0;i<3;i++){ float p=fract(uTime*.3+float(i)/3.); pulse+=exp(-pow((vU-p)*34.,2.))*smoothstep(0.,.08,p); }
  pulse*=uPulse;
  float f=fract(vU*max(uDash,1.));
  float dash=uDash>0.?smoothstep(.0,.08,f)*smoothstep(.55,.47,f):1.;
  float endFade=mix(1.,smoothstep(1.,.2,vU),uFadeEnd);
  float k=uInt*vLum*drawn*dash*endFade;
  vec3 col=uFil*core*(.65+tip*2.6+pulse*3.2)+uAmber*glow*(.26+tip*1.4+pulse*1.5);
  o=vec4(col*k,0.);
}`;
const QUAD_VS = `${HEAD}
in vec2 aQ; uniform vec2 uRes; uniform vec2 uC; uniform vec2 uHalf;
out vec2 vPx; out vec2 vUv;
void main(){ vec2 px=uC+aQ*uHalf; vPx=px; vUv=vec2(aQ.x*.5+.5, aQ.y*.5+.5);
  gl_Position=vec4(px.x/uRes.x*2.-1., 1.-px.y/uRes.y*2., 0., 1.); }`;
const NODE_FS = `${HEAD}
in vec2 vPx; out vec4 o;
uniform vec2 uC; uniform vec2 uHalf; uniform float uR; uniform float uBlur; uniform float uInt; uniform float uFlash; uniform float uPing; uniform float uSel; uniform float uTime; uniform float uPx;
uniform vec3 uAmber; uniform vec3 uFil;
void main(){
  vec2 p=vPx-uC; float r=length(p);
  float win=1.-smoothstep(.55,1.,r/uHalf.x);
  float b=uBlur*uPx*2.2;
  float soft=1./(1.+uBlur*.9);
  float core=exp(-pow(r/(uR*.2+b*.9+uPx),2.))*soft;
  float w=uPx*.9+b*1.3;
  float ring=exp(-pow((r-uR)/w,2.))*.8*soft;
  float halo=exp(-r/(uR*1.5+b*2.5))*.2;
  float pr=uR*(1.+uPing*7.);
  float ping=exp(-pow((r-pr)/(w*1.6),2.))*(1.-uPing)*step(.001,uPing)*.85;
  float a=atan(p.y,p.x)+uTime*.35;
  float gaps=smoothstep(.15,.4,abs(sin(a*2.)));
  float sel=exp(-pow((r-uR*2.05)/(w*.85),2.))*gaps*uSel;
  float flash=uFlash*exp(-r/(uR*2.6))*1.5;
  vec3 col=uFil*core*(1.3+uFlash*2.5)+uAmber*(ring+halo+ping+sel*.95+flash);
  o=vec4(col*uInt*win,0.);
}`;
const EYE_FS = `${HEAD}${NOISE}
in vec2 vUv; out vec4 o;
uniform sampler2D uMask; uniform vec2 uGaze; uniform vec2 uSheen; uniform float uLid; uniform vec2 uOC; uniform float uWake; uniform float uTime; uniform float uCoreI;
uniform vec3 uAmber; uniform vec3 uHot; uniform vec3 uFil; uniform vec3 uGraphite;
void main(){
  vec2 uv=vec2(vUv.x,1.-vUv.y);
  vec4 m=texture(uMask,uv);
  float outer=m.r, ob=m.b;
  float lid=max(uLid,0.);
  float on=step(.004,lid);
  vec2 cuv=uv-uGaze;
  cuv.y=(cuv.y-uOC.y)/max(lid,.004)+uOC.y;
  vec4 cm=texture(uMask,cuv);
  float counter=cm.g*on, cb=cm.a*on;
  float body=clamp(outer-counter,0.,1.);
  float n=vnoise(uv*6.5+uTime*.04)*.6+vnoise(uv*21.-uTime*.07)*.4;
  float rim=clamp((outer-ob)*3.,0.,1.);
  float lip=clamp(cb*1.5,0.,1.)*(1.-counter);
  float shade=mix(.8,1.04,clamp(.5+dot(uv-vec2(.5),vec2(.75,-.75))*1.5,0.,1.));
  // A bevel from the blurred masks; a soft specular that slides as the eye looks around.
  float e=2.5/512.;
  vec2 og=vec2(texture(uMask,uv+vec2(e,0.)).b-texture(uMask,uv-vec2(e,0.)).b, texture(uMask,uv+vec2(0.,e)).b-texture(uMask,uv-vec2(0.,e)).b);
  vec2 cg=on*vec2(texture(uMask,cuv+vec2(e,0.)).a-texture(uMask,cuv-vec2(e,0.)).a, texture(uMask,cuv+vec2(0.,e)).a-texture(uMask,cuv-vec2(0.,e)).a);
  vec3 nrm=normalize(vec3(-(og-cg)*7.,1.));
  vec3 L=normalize(vec3(-.45+uSheen.x*.7,-.55+uSheen.y*.7,.75));
  // Only the bevel catches the light: a flat face would reflect straight back and wash out.
  float bevel=smoothstep(.04,.35,length(og-cg)*7.);
  float spec=pow(max(dot(reflect(-L,nrm),vec3(0.,0.,1.)),0.),14.)*bevel;
  float occl=clamp(dot(nrm.xy,vec2(-.6,-.6))*.5+.5,0.,1.);
  vec3 lit=uAmber*shade*(.9+.16*n)*mix(.82,1.04,occl)+uHot*rim*.26+uFil*lip*.5*uCoreI+uFil*spec*.42;
  vec3 dormant=uGraphite*.85+uAmber*rim*.05;
  vec3 bodyCol=mix(dormant,lit,uWake);
  float halo=ob*(1.-outer)*uWake;
  float inner=counter*(1.-clamp(cb*1.2,0.,1.))*uWake;
  vec3 add=uAmber*halo*halo*.2+uHot*inner*.22;
  o=vec4(bodyCol*body+add*(1.-body),body);
}`;
const SHUTTER_FS = `${HEAD}
in vec2 vPx; out vec4 o;
uniform vec2 uC; uniform float uR; uniform float uClose; uniform float uPx; uniform vec3 uAmber; uniform vec3 uGraphite; uniform vec3 uHot;
void main(){
  vec2 p=vPx-uC; float r=length(p);
  float a=atan(p.y,p.x);
  float seg=1.0471976;
  float rot=uClose*1.15;
  float open=uR*(1.-uClose)*.97;
  float aa=mod(a+rot,seg)-seg*.5;
  float hexR=open/cos(aa);
  float inside=smoothstep(uR+uPx,uR-uPx,r);
  float blade=smoothstep(-uPx,uPx,r-hexR)*inside;
  float tw=mod(a+rot+(r-hexR)/uR*1.25+seg*.5,seg)-seg*.5;
  float seam=exp(-pow(tw*r/(1.1*uPx),2.))*blade*step(hexR,r);
  float edge=exp(-pow((r-hexR)/(1.3*uPx),2.))*inside;
  float ring=exp(-pow((r-uR)/(1.4*uPx),2.));
  float vis=smoothstep(0.,.06,uClose);
  vec3 bladeCol=uGraphite*(.7+.3*(r/uR))+uAmber*.05;
  vec3 light=uAmber*(seam*.9+ring*.9)+uHot*edge*1.6;
  float alpha=blade*vis;
  o=vec4(bladeCol*alpha+light*vis,alpha);
}`;
const COMP_FS = `${HEAD}${NOISE}
in vec2 vUv; out vec4 o;
uniform sampler2D uScene; uniform vec2 uRes; uniform float uTime; uniform float uCA; uniform float uGrain;
void main(){
  vec2 uv=vUv; vec2 dir=uv-.5;
  float k=dot(dir,dir)*uCA;
  vec3 base=texture(uScene,uv).rgb;
  vec3 c=vec3(texture(uScene,uv+dir*k).r, base.g, base.b);
  vec3 knee=vec3(.78);
  vec3 sh=knee+(1.-knee)*(1.-exp(-(c-knee)/(1.-knee)));
  c=mix(c,sh,step(knee,c));
  float vig=smoothstep(1.2,.25,length(dir*vec2(uRes.x/uRes.y,1.)*.92));
  c*=mix(.5,1.,vig);
  c=pow(max(c,0.),vec3(1./2.2));
  float g=h21(gl_FragCoord.xy+fract(uTime*7.31)*vec2(113.,71.))-.5;
  c+=g*uGrain;
  o=vec4(c,1.);
}`;

/* ---------- GL helpers ---------- */
type Program = { p: WebGLProgram; u: Record<string, WebGLUniformLocation | null>; a: Record<string, number> };
function program(gl: WebGL2RenderingContext, vs: string, fs: string): Program {
  const make = (type: number, src: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(`shader: ${gl.getShaderInfoLog(s)}`);
    return s;
  };
  const p = gl.createProgram()!;
  gl.attachShader(p, make(gl.VERTEX_SHADER, vs)); gl.attachShader(p, make(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(`link: ${gl.getProgramInfoLog(p)}`);
  const u: Program["u"] = {}, a: Program["a"] = {};
  for (let i = 0, n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS); i < n; i++) {
    const info = gl.getActiveUniform(p, i)!; u[info.name] = gl.getUniformLocation(p, info.name);
  }
  for (let i = 0, n = gl.getProgramParameter(p, gl.ACTIVE_ATTRIBUTES); i < n; i++) {
    const info = gl.getActiveAttrib(p, i)!; a[info.name] = gl.getAttribLocation(p, info.name);
  }
  return { p, u, a };
}

/* The mark, rasterised once: R outer, G opening, B outer blurred, A opening blurred. */
const MASK = 512, MASK_PAD = MASK * 0.17, MASK_SCALE = (MASK - 2 * MASK_PAD) / MARK_H, MASK_OX = (MASK - MARK_W * MASK_SCALE) / 2;
function boxBlur(src: Float32Array, size: number, radius: number, passes: number): Float32Array {
  // Work on a copy: the sharp mask must survive for the R and G channels.
  let a: Float32Array = Float32Array.from(src), b: Float32Array = new Float32Array(src.length);
  for (let pass = 0; pass < passes; pass++) {
    for (const horizontal of [true, false]) {
      for (let line = 0; line < size; line++) {
        let acc = 0;
        const at = (i: number) => (horizontal ? a[line * size + i] : a[i * size + line]);
        for (let i = -radius; i <= radius; i++) acc += at(Math.min(size - 1, Math.max(0, i)));
        for (let i = 0; i < size; i++) {
          const idx = horizontal ? line * size + i : i * size + line;
          b[idx] = acc / (radius * 2 + 1);
          acc += at(Math.min(size - 1, i + radius + 1)) - at(Math.max(0, i - radius));
        }
      }
      [a, b] = [b, a];
    }
  }
  return a;
}
function rasterMask(): Uint8Array {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = MASK;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  const layer = (d: string) => {
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, MASK, MASK);
    ctx.setTransform(MASK_SCALE, 0, 0, MASK_SCALE, MASK_OX, MASK_PAD);
    ctx.fillStyle = "#fff"; ctx.fill(new Path2D(d));
    const data = ctx.getImageData(0, 0, MASK, MASK).data, out = new Float32Array(MASK * MASK);
    for (let i = 0; i < out.length; i++) out[i] = data[i * 4 + 3] / 255;
    return out;
  };
  const outer = layer(geometry.mark.paths[0].d), opening = layer(geometry.mark.paths[1].d);
  const outerBlur = boxBlur(outer, MASK, 9, 3), openingBlur = boxBlur(opening, MASK, 6, 3);
  const rgba = new Uint8Array(MASK * MASK * 4);
  for (let i = 0; i < outer.length; i++) {
    rgba[i * 4] = outer[i] * 255; rgba[i * 4 + 1] = opening[i] * 255;
    rgba[i * 4 + 2] = outerBlur[i] * 255; rgba[i * 4 + 3] = openingBlur[i] * 255;
  }
  return rgba;
}

/* ---------- the scene ---------- */
export function createEyeScene(canvas: HTMLCanvasElement, options: SceneOptions): EyeScene {
  const gl = canvas.getContext("webgl2", {
    antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false,
    powerPreference: "high-performance", preserveDrawingBuffer: !!options.capture,
  });
  if (!gl) throw new Error("WebGL2 unavailable");
  const debug = gl.getExtension("WEBGL_debug_renderer_info");
  const renderer = debug ? String(gl.getParameter(debug.UNMASKED_RENDERER_WEBGL)) : "unknown";
  const half = !!gl.getExtension("EXT_color_buffer_float") || !!gl.getExtension("EXT_color_buffer_half_float");
  const low = options.tier === "low";

  const P = {
    field: program(gl, FULL_VS, FIELD_FS),
    dust: program(gl, DUST_VS, DUST_FS),
    ribbon: program(gl, RIBBON_VS, RIBBON_FS),
    node: program(gl, QUAD_VS, NODE_FS),
    eye: program(gl, QUAD_VS, EYE_FS),
    shutter: program(gl, QUAD_VS, SHUTTER_FS),
    comp: program(gl, FULL_VS, COMP_FS),
  };
  const emptyVao = gl.createVertexArray()!;

  // Unit quad for billboards.
  const quadVao = gl.createVertexArray()!;
  gl.bindVertexArray(quadVao);
  const quadBuf = gl.createBuffer()!;
  gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  for (const prog of [P.node, P.eye, P.shutter]) {
    const loc = prog.a.aQ; if (loc === undefined || loc < 0) continue;
    gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  }

  // Dust motes. Deterministic positions so stills match the live scene.
  const DUST = low ? 150 : 380;
  const dustData = new Float32Array(DUST * 4);
  let seed = 7;
  const rand = () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };
  for (let i = 0; i < DUST; i++) {
    dustData.set([(rand() - 0.5) * 16, (rand() - 0.5) * 10, -7 + rand() * 11, rand()], i * 4);
  }
  const dustVao = gl.createVertexArray()!;
  gl.bindVertexArray(dustVao);
  const dustBuf = gl.createBuffer()!;
  gl.bindBuffer(gl.ARRAY_BUFFER, dustBuf);
  gl.bufferData(gl.ARRAY_BUFFER, dustData, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(P.dust.a.aP); gl.vertexAttribPointer(P.dust.a.aP, 4, gl.FLOAT, false, 0, 0);

  // Ribbons share one dynamic buffer.
  const RIB_FLOATS = 7, RIB_MAX = 96;
  const ribVao = gl.createVertexArray()!;
  gl.bindVertexArray(ribVao);
  const ribBuf = gl.createBuffer()!;
  gl.bindBuffer(gl.ARRAY_BUFFER, ribBuf);
  gl.bufferData(gl.ARRAY_BUFFER, RIB_MAX * 2 * RIB_FLOATS * 4 * 14, gl.DYNAMIC_DRAW);
  const ribAttrs: [string, number][] = [["aPx", 2], ["aU", 1], ["aSide", 1], ["aHalf", 1], ["aCore", 1], ["aLum", 1]];
  let offset = 0;
  for (const [name, size] of ribAttrs) {
    const loc = P.ribbon.a[name];
    gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, size, gl.FLOAT, false, RIB_FLOATS * 4, offset * 4);
    offset += size;
  }
  gl.bindVertexArray(null);
  const ribScratch = new Float32Array(RIB_MAX * 2 * RIB_FLOATS);

  // Mask texture.
  const maskTex = gl.createTexture()!;
  gl.bindTexture(gl.TEXTURE_2D, maskTex);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, MASK, MASK, 0, gl.RGBA, gl.UNSIGNED_BYTE, rasterMask());
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  // Offscreen buffer.
  const fbo = gl.createFramebuffer()!, fboTex = gl.createTexture()!;
  let fw = 0, fh = 0, cssW = 0, cssH = 0, dpr = 1;
  let renderScale = low ? 0.72 : 1;
  const DPR_CAP = low ? 1.25 : 1.75;
  function resize() {
    cssW = Math.max(1, canvas.clientWidth); cssH = Math.max(1, canvas.clientHeight);
    dpr = Math.min(window.devicePixelRatio || 1, DPR_CAP);
    const w = Math.round(cssW * dpr), h = Math.round(cssH * dpr);
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    const nfw = Math.max(1, Math.round(w * renderScale)), nfh = Math.max(1, Math.round(h * renderScale));
    if (nfw === fw && nfh === fh) return;
    fw = nfw; fh = nfh;
    gl!.bindTexture(gl!.TEXTURE_2D, fboTex);
    gl!.texImage2D(gl!.TEXTURE_2D, 0, half ? gl!.RGBA16F : gl!.RGBA8, fw, fh, 0, gl!.RGBA, half ? gl!.HALF_FLOAT : gl!.UNSIGNED_BYTE, null);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MIN_FILTER, gl!.LINEAR);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_MAG_FILTER, gl!.LINEAR);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_S, gl!.CLAMP_TO_EDGE);
    gl!.texParameteri(gl!.TEXTURE_2D, gl!.TEXTURE_WRAP_T, gl!.CLAMP_TO_EDGE);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, fbo);
    gl!.framebufferTexture2D(gl!.FRAMEBUFFER, gl!.COLOR_ATTACHMENT0, gl!.TEXTURE_2D, fboTex, 0);
    if (gl!.checkFramebufferStatus(gl!.FRAMEBUFFER) !== gl!.FRAMEBUFFER_COMPLETE) throw new Error("framebuffer incomplete");
  }
  resize();

  /* ---- motion state ---- */
  const start = performance.now();
  let last = start;
  const gaze = { x: 0, y: 0, vx: 0, vy: 0 };
  const parallax = { x: 0, y: 0 };
  let pointerSeen = -1e9;
  const onPointer = (event: PointerEvent) => {
    if (event.pointerType && event.pointerType !== "mouse" && event.pointerType !== "pen") return;
    sceneState.pointer = { x: event.clientX, y: event.clientY };
    pointerSeen = performance.now();
  };
  const onLeave = (event: PointerEvent) => { if (!event.relatedTarget) sceneState.pointer = null; };
  window.addEventListener("pointermove", onPointer, { passive: true });
  window.addEventListener("pointerout", onLeave);

  const frameTimes: number[] = [];
  let frames = 0, slowStreak = 0, raf = 0, running = false, disposed = false, firstFrame = false;

  const project = (vp: Float32Array, p: V3) => {
    const x = vp[0] * p[0] + vp[4] * p[1] + vp[8] * p[2] + vp[12];
    const y = vp[1] * p[0] + vp[5] * p[1] + vp[9] * p[2] + vp[13];
    const w = vp[3] * p[0] + vp[7] * p[1] + vp[11] * p[2] + vp[15];
    return { x: (x / w * 0.5 + 0.5) * fw, y: (0.5 - y / w * 0.5) * fh, w };
  };

  function setVec3(prog: Program, name: string, v: V3) { const l = prog.u[name]; if (l) gl!.uniform3f(l, v[0], v[1], v[2]); }

  function drawRibbon(points: { x: number; y: number; w: number }[], core: (i: number) => number, lum: (i: number) => number, uni: { draw: number; int: number; pulse?: number; dash?: number; fadeEnd?: number }, time: number) {
    const n = Math.min(points.length, RIB_MAX);
    if (n < 2 || uni.int <= 0.001 || uni.draw <= 0.001) return;
    const pxu = fh / 900 * 1.0 + 0.4;
    let k = 0;
    for (let i = 0; i < n; i++) {
      const a = points[Math.max(0, i - 1)], b = points[Math.min(n - 1, i + 1)];
      let nx = -(b.y - a.y), ny = b.x - a.x;
      const l = Math.hypot(nx, ny) || 1; nx /= l; ny /= l;
      const c = core(i) * pxu;
      const halfW = Math.max(c * 9, 26 * pxu);
      const u = i / (n - 1);
      for (const side of [-1, 1]) {
        ribScratch[k++] = points[i].x + nx * halfW * side; ribScratch[k++] = points[i].y + ny * halfW * side;
        ribScratch[k++] = u; ribScratch[k++] = side; ribScratch[k++] = halfW; ribScratch[k++] = c; ribScratch[k++] = lum(i);
      }
    }
    gl!.bufferSubData(gl!.ARRAY_BUFFER, 0, ribScratch, 0, k);
    gl!.uniform1f(P.ribbon.u.uDraw, uni.draw); gl!.uniform1f(P.ribbon.u.uInt, uni.int);
    gl!.uniform1f(P.ribbon.u.uPulse, uni.pulse ?? 0); gl!.uniform1f(P.ribbon.u.uDash, uni.dash ?? 0);
    gl!.uniform1f(P.ribbon.u.uFadeEnd, uni.fadeEnd ?? 0); gl!.uniform1f(P.ribbon.u.uTime, time);
    gl!.drawArrays(gl!.TRIANGLE_STRIP, 0, n * 2);
  }

  function bezier(a: V3, b: V3, c: V3, d: V3, t: number): V3 {
    const m = 1 - t, k0 = m * m * m, k1 = 3 * m * m * t, k2 = 3 * m * t * t, k3 = t * t * t;
    return [k0 * a[0] + k1 * b[0] + k2 * c[0] + k3 * d[0], k0 * a[1] + k1 * b[1] + k2 * c[1] + k3 * d[1], k0 * a[2] + k1 * b[2] + k2 * c[2] + k3 * d[2]];
  }

  function render(now: number) {
    if (disposed) return;
    const dt = Math.min(0.05, (now - last) / 1000); last = now;
    const time = sceneState.timeOverride ?? (now - start) / 1000;
    const t = sceneState.introOverride ?? time;
    const s = sceneState;
    resize();

    const layout = cssW / cssH < 0.9 ? NARROW : WIDE;
    const pxu = fh / 900 + 0.4;

    /* ---- phases ---- */
    const heroOut = quartInOut(s.heroOut);
    const story = s.story;
    const dsc = span(story, 0, 0.25), sel = span(story, 0.25, 0.5), grant = span(story, 0.5, 0.75), run = span(story, 0.75, 1);
    const pull = quartInOut(s.pull);
    const cta = quintOut(s.cta);
    const visible = Math.max(1 - s.sink, s.cta > 0 ? smooth(0, 0.35, s.cta) : 0);
    canvas.style.opacity = visible.toFixed(3);

    // Keep the eye clear of the headline's measured right edge (the type sits in a centred
    // container, the eye is framed to the viewport, so very wide screens would overlap).
    const clear = (f: Framing, edge: number) => {
      if (layout !== WIDE || edge <= 0) return f;
      const eyePx = (f.eyeH * (cssH / (2 * TAN))) / f.camZ;
      const need = ((edge + eyePx * 0.62) / cssW) * 2 - 1;
      return { ...f, ex: Math.min(0.64, Math.max(f.ex, need)) };
    };
    const heroF = clear(layout.hero, s.clearOf.hero), ctaF = clear(layout.cta, s.clearOf.cta);
    let F = mixFraming(heroF, layout.story, heroOut);
    F = mixFraming(F, layout.pull, pull);
    if (cta > 0) F = mixFraming(F, ctaF, cta);

    /* ---- intro ---- */
    const fieldIn = smooth(0, 0.9, t);
    const wakeRaw = expoOut((t - 0.25) / 1.15);
    const flicker = t < 1.4 ? 0.78 + 0.22 * Math.sin(t * 41) * Math.sin(t * 17.3) : 1;
    const wake = wakeRaw * flicker;
    let lid = expoOut((t - 0.95) / 1.5);
    if (s.blinkAt > 0) {
      const bt = (now - s.blinkAt) / 1000;
      if (bt >= 0 && bt < 0.42) lid *= bt < 0.12 ? 1 - quintOut(bt / 0.12) * 0.97 : 0.03 + 0.97 * expoOut((bt - 0.12) / 0.3);
    }
    const ignite = layout.nodes.map((_, i) => expoOut((t - (1.95 + i * 0.42)) / 0.75));
    const flash = layout.nodes.map((_, i) => { const d = t - (1.95 + i * 0.42); return d > 0 ? Math.exp(-d * 4.5) : 0; });
    const threadIntro = layout.nodes.map((_, i) => quintOut((t - (2.25 + i * 0.42)) / 1.15));

    /* ---- camera ---- */
    const aspect = fw / fh;
    const hasPointer = !!s.pointer && now - pointerSeen < 6000;
    const pnx = hasPointer ? (s.pointer!.x / cssW) * 2 - 1 : 0, pny = hasPointer ? (s.pointer!.y / cssH) * 2 - 1 : 0;
    const pk = 1 - Math.exp(-dt * 2.2);
    parallax.x += (pnx - parallax.x) * pk; parallax.y += (pny - parallax.y) * pk;
    const orbit = Math.sin(story * Math.PI) * 0.9 * (1 - pull);
    const eyeW: V3 = [F.ex * TAN * 10 * aspect, F.ey * TAN * 10, 0];
    const camPos: V3 = [eyeW[0] * 0.35 + parallax.x * 0.45 + orbit, eyeW[1] * 0.25 - parallax.y * 0.3, F.camZ];
    const camTgt: V3 = [eyeW[0] * 0.35 + orbit * 0.55, eyeW[1] * 0.25, 0];
    const view = lookAt(camPos, camTgt);
    // Recentre so the eye lands on its framing anchor despite the orbit and parallax.
    const proj = perspective(FOV, aspect, 0.1, 80);
    const vp = mul(proj, view);
    {
      // Recentre in clip space (x' = x + ox * w), so the eye lands on its framing anchor
      // whatever the orbit and parallax did; the pull-back lets the camera drift instead.
      const e = project(vp, eyeW);
      const want = { x: (F.ex * 0.5 + 0.5) * fw, y: (0.5 - F.ey * 0.5) * fh };
      const keep = 1 - pull * 0.85;
      const ox = ((want.x - e.x) / fw) * 2 * keep, oy = (-(want.y - e.y) / fh) * 2 * keep;
      for (let c = 0; c < 4; c++) { vp[c * 4] += ox * vp[c * 4 + 3]; vp[c * 4 + 1] += oy * vp[c * 4 + 3]; }
    }
    const eyeP = project(vp, eyeW);
    const pxPerWorld = fh / (2 * TAN);
    const eyePxH = (F.eyeH * pxPerWorld) / eyeP.w;

    /* ---- nodes ---- */
    const ringScale = F.ring;
    const selAmt = quintOut(sel) * (1 - pull);
    const nodeW: V3[] = layout.nodes.map((o, i) => {
      const drift: V3 = [Math.sin(time * 0.31 + i * 1.7) * 0.06, Math.cos(time * 0.27 + i * 2.3) * 0.06, 0];
      let p: V3 = [eyeW[0] + o[0] * ringScale + drift[0], eyeW[1] + o[1] * ringScale + drift[1], o[2] * ringScale * 1.4];
      if (i === 0 && selAmt > 0) {
        const f: V3 = [eyeW[0] + layout.front[0] * ringScale, eyeW[1] + layout.front[1] * ringScale, layout.front[2] * Math.max(ringScale, 0.55)];
        p = mix3(p, f, selAmt);
      }
      return p;
    });
    const nodeP = nodeW.map((p) => project(vp, p));
    const focus = mix(eyeP.w, nodeP[0].w, selAmt);
    const dofK = low ? 0.55 : 0.75;
    const blurOf = (w: number) => Math.min(3.2, Math.abs(w - focus) * dofK);
    // Discover pings: one per table row, in the row order.
    const found = layout.nodes.map((_, i) => span(dsc, 0.08 + i * 0.17, 0.08 + i * 0.17 + 0.06));
    const ping = layout.nodes.map((_, i) => { const p = span(dsc, 0.08 + i * 0.17, 0.08 + i * 0.17 + 0.22); return p > 0 && p < 1 ? p : 0; });
    const dim = layout.nodes.map((_, i) => (i === 0 ? 1 : mix(1, 0.36, selAmt)) * mix(1, 0.5, cta));

    /* ---- gaze ---- */
    let target: { x: number; y: number } | null = null;
    if (s.lookAt && s.cta > 0.4) target = { x: s.lookAt.x * (fw / cssW), y: s.lookAt.y * (fh / cssH) };
    else if (hasPointer) target = { x: s.pointer!.x * (fw / cssW), y: s.pointer!.y * (fh / cssH) };
    else {
      let idx = -1;
      if (story > 0 && story < 1) idx = sel > 0 ? 0 : Math.min(4, Math.floor(dsc * 5.2));
      else if (t < 4.6) ignite.forEach((v, i) => { if (v > 0.02) idx = i; });
      else idx = Math.floor(time / 2.6) % 5;
      if (pull > 0.3) idx = -1;
      if (idx >= 0) target = { x: nodeP[idx].x, y: nodeP[idx].y };
    }
    let gx = 0, gy = 0;
    if (target) {
      const dx = target.x - eyeP.x, dy = target.y - eyeP.y, d = Math.hypot(dx, dy);
      const strength = Math.min(1, d / (eyePxH * 1.35));
      if (d > 0) { gx = (dx / d) * strength * GAZE_X; gy = (dy / d) * strength * GAZE_Y; }
    }
    const k = 70, damp = 9.5;
    const steps = Math.max(1, Math.ceil(dt / 0.008));
    for (let i = 0; i < steps; i++) {
      const h = dt / steps;
      gaze.vx += (k * (gx - gaze.x) - damp * gaze.vx) * h; gaze.vy += (k * (gy - gaze.y) - damp * gaze.vy) * h;
      gaze.x += gaze.vx * h; gaze.y += gaze.vy * h;
    }
    if (sceneState.timeOverride !== null) { gaze.x = gx; gaze.y = gy; }

    /* ---- core: behind the eye, centred on the opening ---- */
    const openDx = ((OPEN_CX - MARK_W / 2) / MARK_H) * F.eyeH, openDy = -((OPEN_CY - MARK_H / 2) / MARK_H) * F.eyeH;
    const coreW: V3 = [eyeW[0] + openDx + (gaze.x / MARK_H) * F.eyeH * 0.6, eyeW[1] + openDy - (gaze.y / MARK_H) * F.eyeH * 0.6, -0.55 * (F.eyeH / 2.35)];
    const coreP = project(vp, coreW);
    const connected = threadIntro.reduce((a, v) => a + v, 0) / 5;
    const runPulse = smooth(0, 0.12, run) * (1 - smooth(0.5, 1, pull));
    const coreI = 0.35 + connected * 0.5 + runPulse * (0.35 + 0.25 * Math.sin(time * 6.2));

    /* ---- draw ---- */
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, fbo);
    gl!.viewport(0, 0, fw, fh);
    gl!.disable(gl!.BLEND);
    gl!.useProgram(P.field.p);
    gl!.bindVertexArray(emptyVao);
    gl!.uniform2f(P.field.u.uRes, fw, fh);
    gl!.uniform2f(P.field.u.uEye, eyeP.x, eyeP.y);
    gl!.uniform1f(P.field.u.uEyeR, eyePxH * 0.75);
    gl!.uniform1f(P.field.u.uLight, wake * fieldIn * (0.85 + coreI * 0.2));
    gl!.uniform1f(P.field.u.uTime, time);
    setVec3(P.field, "uUnlit", C.unlit); setVec3(P.field, "uAmber", C.amber);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);

    gl!.enable(gl!.BLEND);
    gl!.blendFunc(gl!.ONE, gl!.ONE);

    // dust
    gl!.useProgram(P.dust.p);
    gl!.bindVertexArray(dustVao);
    gl!.uniformMatrix4fv(P.dust.u.uVP, false, vp);
    gl!.uniform1f(P.dust.u.uTime, time); gl!.uniform1f(P.dust.u.uFocus, focus);
    gl!.uniform1f(P.dust.u.uPxW, pxPerWorld); gl!.uniform3f(P.dust.u.uEyeW, eyeW[0], eyeW[1], eyeW[2]);
    gl!.uniform1f(P.dust.u.uAmt, fieldIn * (0.7 + wake * 0.6));
    setVec3(P.dust, "uAmber", C.amber); setVec3(P.dust, "uFil", C.filament);
    gl!.drawArrays(gl!.POINTS, 0, DUST);

    // ribbons
    gl!.useProgram(P.ribbon.p);
    gl!.bindVertexArray(ribVao);
    gl!.bindBuffer(gl!.ARRAY_BUFFER, ribBuf);
    gl!.uniform2f(P.ribbon.u.uRes, fw, fh); gl!.uniform1f(P.ribbon.u.uPx, pxu);
    setVec3(P.ribbon, "uAmber", C.amber); setVec3(P.ribbon, "uFil", C.filament);

    const anchors: Record<string, Anchor> = {};

    // machine outline: a laptop drawn around everything during the pull-back
    const outlineDraw = quintOut(span(s.pull, 0.12, 0.6)) * (1 - cta);
    const providerDraw = quintOut(span(s.pull, 0.42, 0.85)) * (1 - cta);
    if (outlineDraw > 0.001) {
      const [mx, my] = layout.machine;
      const cx = eyeW[0] - (layout === WIDE ? 0.15 : 0) * ringScale, cy = eyeW[1] + 0.05 * ringScale;
      const hw = 3.4 * mx * ringScale, hh = 2.45 * my * ringScale, z = -0.9 * ringScale, r = 0.34 * ringScale;
      const pts: V3[] = [];
      const corner = (x0: number, y0: number, a0: number) => { for (let j = 0; j <= 6; j++) { const a = a0 + (j / 6) * Math.PI / 2; pts.push([x0 + Math.cos(a) * r, y0 + Math.sin(a) * r, z]); } };
      // start at bottom centre, go counter-clockwise, close back at bottom centre
      pts.push([cx, cy - hh, z]);
      corner(cx + hw - r, cy - hh + r, -Math.PI / 2);
      corner(cx + hw - r, cy + hh - r, 0);
      corner(cx - hw + r, cy + hh - r, Math.PI / 2);
      corner(cx - hw + r, cy - hh + r, Math.PI);
      pts.push([cx, cy - hh, z]);
      const dense: V3[] = [];
      for (let i = 0; i < pts.length - 1; i++) for (let j = 0; j < 3; j++) dense.push(mix3(pts[i], pts[i + 1], j / 3));
      dense.push(pts[pts.length - 1]);
      const proj2 = dense.map((p) => project(vp, p));
      drawRibbon(proj2, () => 1.1, () => 0.9, { draw: outlineDraw, int: 0.85 }, time);
      // deck
      const deck: V3[] = [[cx - hw * 1.12, cy - hh - 0.28 * ringScale, z], [cx - hw * 0.25, cy - hh - 0.28 * ringScale, z], [cx - hw * 0.2, cy - hh - 0.2 * ringScale, z], [cx + hw * 0.2, cy - hh - 0.2 * ringScale, z], [cx + hw * 0.25, cy - hh - 0.28 * ringScale, z], [cx + hw * 1.12, cy - hh - 0.28 * ringScale, z]];
      const deckDense: V3[] = [];
      for (let i = 0; i < deck.length - 1; i++) for (let j = 0; j < 8; j++) deckDense.push(mix3(deck[i], deck[i + 1], j / 8));
      deckDense.push(deck[deck.length - 1]);
      drawRibbon(deckDense.map((p) => project(vp, p)), () => 1.1, () => 0.9, { draw: span(outlineDraw, 0.55, 1), int: 0.85 }, time);
      const tl = project(vp, [cx - hw, cy + hh, z]);
      anchors.machine = { x: tl.x, y: tl.y, alpha: smooth(0.5, 0.9, outlineDraw), scale: 1 };

      // provider-bound lines: they leave the machine. Ollama's route stays on the device.
      if (providerDraw > 0.001) {
        [0, 1, 2, 3].forEach((i) => {
          const from = nodeW[i];
          const dir = [from[0] - cx, from[1] - cy];
          const l = Math.hypot(dir[0], dir[1]) || 1;
          const reach = Math.max(hw, hh) * 1.55;
          const to: V3 = [cx + (dir[0] / l) * reach * 1.25, cy + (dir[1] / l) * reach, from[2]];
          const line: V3[] = [];
          for (let j = 0; j <= 40; j++) line.push(mix3(from, to, j / 40));
          const pl = line.map((p) => project(vp, p));
          drawRibbon(pl, () => 0.8, () => 0.8, { draw: providerDraw, int: 0.6, dash: 22, fadeEnd: 1 }, time);
          if (i === 2) { const a = pl[30]; anchors.provider = { x: a.x, y: a.y, alpha: smooth(0.4, 0.9, providerDraw), scale: 1 }; }
        });
        anchors.local = { x: nodeP[4].x, y: nodeP[4].y, alpha: smooth(0.3, 0.8, providerDraw), scale: 1 };
      }
    }

    // threads node -> core
    nodeW.forEach((nw, i) => {
      const draw = Math.max(threadIntro[i], 0);
      if (draw <= 0.001) return;
      const toCore = [coreW[0] - nw[0], coreW[1] - nw[1], coreW[2] - nw[2]];
      const out = [nw[0] - eyeW[0], nw[1] - eyeW[1]];
      const ol = Math.hypot(out[0], out[1]) || 1;
      const rimPt: V3 = [eyeW[0] + (out[0] / ol) * F.eyeH * 0.62, eyeW[1] + (out[1] / ol) * F.eyeH * 0.62, coreW[2] + 0.3];
      const sag = 0.35 * ringScale * (i % 2 ? 1 : -1);
      const c1: V3 = [nw[0] + toCore[0] * 0.3 - out[1] / ol * sag, nw[1] + toCore[1] * 0.3 + out[0] / ol * sag, nw[2] + toCore[2] * 0.3];
      const pts: { x: number; y: number; w: number }[] = [];
      for (let j = 0; j <= 56; j++) pts.push(project(vp, bezier(nw, c1, rimPt, coreW, j / 56)));
      const selected = i === 0;
      const strength = (0.45 + found[i] * 0.55) * dim[i] * (selected ? 1 + selAmt * 0.25 + runPulse * 0.3 : 1);
      drawRibbon(
        pts,
        (j) => 0.85 + blurOf(pts[j].w) * 1.4 + (selected ? selAmt * 0.3 : 0),
        (j) => 1 / (1 + blurOf(pts[j].w) * 0.7),
        { draw, int: strength * (0.75 + 0.25 * fieldIn), pulse: selected ? runPulse : 0 },
        time,
      );
    });

    // billboards
    gl!.bindVertexArray(quadVao);
    const node = (p: { x: number; y: number }, radius: number, blur: number, intensity: number, extra: { flash?: number; ping?: number; sel?: number }) => {
      if (intensity <= 0.002) return;
      const halfPx = Math.max(radius * 8, radius * (1 + (extra.ping ?? 0) * 7) * 1.4 + 6, 40 * pxu);
      gl!.uniform2f(P.node.u.uC, p.x, p.y); gl!.uniform2f(P.node.u.uHalf, halfPx, halfPx);
      gl!.uniform1f(P.node.u.uR, radius); gl!.uniform1f(P.node.u.uBlur, blur); gl!.uniform1f(P.node.u.uInt, intensity);
      gl!.uniform1f(P.node.u.uFlash, extra.flash ?? 0); gl!.uniform1f(P.node.u.uPing, extra.ping ?? 0); gl!.uniform1f(P.node.u.uSel, extra.sel ?? 0);
      gl!.drawArrays(gl!.TRIANGLE_STRIP, 0, 4);
    };
    gl!.useProgram(P.node.p);
    gl!.uniform2f(P.node.u.uRes, fw, fh); gl!.uniform1f(P.node.u.uTime, time); gl!.uniform1f(P.node.u.uPx, pxu);
    setVec3(P.node, "uAmber", C.amber); setVec3(P.node, "uFil", C.filament);
    // core
    node(coreP, (0.07 * pxPerWorld) / coreP.w, 0, wake * coreI * 1.4, { flash: runPulse * 0.5 });
    // nodes behind the eye
    const order = nodeP.map((p, i) => ({ p, i })).sort((a, b) => b.p.w - a.p.w);
    const drawNode = ({ p, i }: { p: { x: number; y: number; w: number }; i: number }) => {
      const r = (0.085 * pxPerWorld) / p.w;
      node(p, r, blurOf(p.w), ignite[i] * dim[i] * (0.8 + found[i] * 0.4), { flash: flash[i] + found[i] * (1 - found[i]) * 2, ping: ping[i], sel: i === 0 ? selAmt : 0 });
    };
    order.filter(({ p }) => p.w >= eyeP.w).forEach(drawNode);

    // the eye
    gl!.blendFunc(gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
    gl!.useProgram(P.eye.p);
    gl!.activeTexture(gl!.TEXTURE0); gl!.bindTexture(gl!.TEXTURE_2D, maskTex);
    gl!.uniform1i(P.eye.u.uMask, 0);
    const halfEye = (eyePxH / 2) * (MASK / (MASK - 2 * MASK_PAD));
    gl!.uniform2f(P.eye.u.uRes, fw, fh); gl!.uniform2f(P.eye.u.uC, eyeP.x, eyeP.y); gl!.uniform2f(P.eye.u.uHalf, halfEye, halfEye);
    gl!.uniform2f(P.eye.u.uGaze, (gaze.x * MASK_SCALE) / MASK, (gaze.y * MASK_SCALE) / MASK);
    gl!.uniform2f(P.eye.u.uSheen, gaze.x / GAZE_X, gaze.y / GAZE_Y);
    gl!.uniform1f(P.eye.u.uLid, lid);
    gl!.uniform2f(P.eye.u.uOC, (MASK_OX + OPEN_CX * MASK_SCALE) / MASK, (MASK_PAD + OPEN_CY * MASK_SCALE) / MASK);
    gl!.uniform1f(P.eye.u.uWake, wake * fieldIn); gl!.uniform1f(P.eye.u.uTime, time); gl!.uniform1f(P.eye.u.uCoreI, coreI);
    setVec3(P.eye, "uAmber", C.amber); setVec3(P.eye, "uHot", C.hot); setVec3(P.eye, "uFil", C.filament); setVec3(P.eye, "uGraphite", C.graphite);
    gl!.drawArrays(gl!.TRIANGLE_STRIP, 0, 4);

    // nodes in front of the eye
    gl!.blendFunc(gl!.ONE, gl!.ONE);
    gl!.useProgram(P.node.p);
    order.filter(({ p }) => p.w < eyeP.w).forEach(drawNode);

    // consent shutter over the selected node
    const shut = grant < 0.5 ? quintOut(span(grant, 0.06, 0.42)) : 1 - quintOut(span(grant, 0.62, 0.95));
    if (shut > 0.002 && pull < 0.5) {
      gl!.blendFunc(gl!.ONE, gl!.ONE_MINUS_SRC_ALPHA);
      gl!.useProgram(P.shutter.p);
      const r = (0.36 * pxPerWorld) / nodeP[0].w;
      gl!.uniform2f(P.shutter.u.uRes, fw, fh); gl!.uniform2f(P.shutter.u.uC, nodeP[0].x, nodeP[0].y);
      gl!.uniform2f(P.shutter.u.uHalf, r * 1.3, r * 1.3); gl!.uniform1f(P.shutter.u.uR, r);
      gl!.uniform1f(P.shutter.u.uClose, shut); gl!.uniform1f(P.shutter.u.uPx, pxu);
      setVec3(P.shutter, "uAmber", C.amber); setVec3(P.shutter, "uGraphite", C.graphite); setVec3(P.shutter, "uHot", C.hot);
      gl!.drawArrays(gl!.TRIANGLE_STRIP, 0, 4);
    }

    // composite
    gl!.disable(gl!.BLEND);
    gl!.bindFramebuffer(gl!.FRAMEBUFFER, null);
    gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.useProgram(P.comp.p);
    gl!.bindVertexArray(emptyVao);
    gl!.activeTexture(gl!.TEXTURE0); gl!.bindTexture(gl!.TEXTURE_2D, fboTex);
    gl!.uniform1i(P.comp.u.uScene, 0);
    gl!.uniform2f(P.comp.u.uRes, canvas.width, canvas.height);
    gl!.uniform1f(P.comp.u.uTime, time); gl!.uniform1f(P.comp.u.uCA, 0); gl!.uniform1f(P.comp.u.uGrain, 0.05);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);

    /* ---- labels ---- */
    const toCss = fw / cssW;
    const labelOut = 1 - pull * 0.55;
    layout.nodes.forEach((_, i) => {
      const p = nodeP[i], r = (0.085 * pxPerWorld) / p.w;
      // Labels sit on the side away from the eye, so they never lie on their own thread;
      // they flip back if that side would run off screen.
      let side = p.x < eyeP.x ? -1 : 1;
      if (side > 0 && p.x + 150 * toCss > fw) side = -1;
      if (side < 0 && p.x - 150 * toCss < 0) side = 1;
      anchors[`node${i}`] = { x: (p.x + side * r * 2.6) / toCss, y: p.y / toCss, alpha: ignite[i] * dim[i] * labelOut * (1 / (1 + blurOf(p.w) * 0.5)), scale: side };
    });
    for (const key of ["machine", "provider", "local"]) {
      const a = anchors[key]; if (a) { a.x /= toCss; a.y /= toCss; }
    }
    anchors.eye = { x: eyeP.x / toCss, y: eyeP.y / toCss, alpha: 1, scale: eyePxH / toCss };
    anchors.stage = { x: 0, y: 0, alpha: visible, scale: 1 };
    anchorSink.write?.(anchors);

    /* ---- adaptive quality ---- */
    frames++;
    if (frames > 2) {
      const ms = dt * 1000;
      frameTimes.push(ms); if (frameTimes.length > 240) frameTimes.shift();
      slowStreak = ms > 24 ? slowStreak + 1 : Math.max(0, slowStreak - 1);
      if (slowStreak > 45 && renderScale > 0.5 && !options.capture) { renderScale = Math.max(0.5, renderScale - 0.15); fw = 0; slowStreak = 0; }
    }
    if (!firstFrame) { firstFrame = true; options.onFirstFrame?.(); }
  }

  function loop(now: number) {
    raf = 0;
    if (!running) return;
    render(now);
    // Stop drawing while the scene is sunk under the documentation sections.
    const idle = sceneState.sink >= 0.999 && sceneState.cta <= 0;
    raf = idle ? 0 : requestAnimationFrame(loop);
    if (idle) idleWatch();
  }
  let idleTimer = 0;
  function idleWatch() {
    window.clearTimeout(idleTimer);
    idleTimer = window.setTimeout(() => {
      if (!running) return;
      if (sceneState.sink < 0.999 || sceneState.cta > 0) { last = performance.now(); raf = requestAnimationFrame(loop); }
      else idleWatch();
    }, 120);
  }
  function play() { if (running || disposed) return; running = true; last = performance.now(); raf = requestAnimationFrame(loop); }
  function pause() { running = false; if (raf) cancelAnimationFrame(raf); raf = 0; window.clearTimeout(idleTimer); }
  const onVisibility = () => (document.hidden ? pause() : play());
  document.addEventListener("visibilitychange", onVisibility);
  const onLost = (event: Event) => { event.preventDefault(); pause(); options.onFailure?.("context lost"); };
  canvas.addEventListener("webglcontextlost", onLost);
  play();

  const stats = (): SceneStats => {
    const sorted = [...frameTimes].sort((a, b) => a - b);
    return {
      frames, avgMs: sorted.length ? sorted.reduce((a, b) => a + b, 0) / sorted.length : 0,
      p95Ms: sorted.length ? sorted[Math.floor(sorted.length * 0.95)] : 0, renderScale, tier: options.tier, renderer,
    };
  };
  return {
    stats,
    dispose() {
      disposed = true; pause();
      document.removeEventListener("visibilitychange", onVisibility);
      canvas.removeEventListener("webglcontextlost", onLost);
      window.removeEventListener("pointermove", onPointer);
      window.removeEventListener("pointerout", onLeave);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    },
  };
}
