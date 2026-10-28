/** One deterministic physical scene for the site, poster, and offline film frames. */
import * as THREE from 'three';
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js';
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';
import { BokehPass } from 'three/examples/jsm/postprocessing/BokehPass.js';
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js';
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';
import { brand } from '../brand.generated';
import { LAYERS } from '../data/layers';

export type StudioFrame = { seconds: number; spread: number; intro?: number; pointer?: [number, number]; active?: number; framing?: 'hero' };
export type StudioOptions = { width: number; height: number; dpr: number; light: boolean; assetBase?: string };
const W = 6.4, H = W * 1000 / 1440, GAP = 0.92;
const clamp = THREE.MathUtils.clamp;
const smooth = (x: number) => { const t = clamp(x, 0, 1); return t * t * (3 - 2 * t); };
/** Analytic damped spring: deterministic at arbitrary timestamps, including offline seeking. */
export function spring(seconds: number) {
  if (seconds <= 0) return 0;
  const damping = 9, omega = Math.sqrt(120 - damping * damping);
  return 1 - Math.exp(-damping * seconds) * (Math.cos(omega * seconds) + damping / omega * Math.sin(omega * seconds));
}
function line(points: THREE.Vector3[], color: string, opacity = 1) {
  return new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), new THREE.LineBasicMaterial({ color, transparent: true, opacity, toneMapped: false }));
}

export async function createStudio(canvas: HTMLCanvasElement, options: StudioOptions) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false, powerPreference: 'high-performance', preserveDrawingBuffer: true });
  renderer.setPixelRatio(Math.min(options.dpr, 2));
  renderer.setSize(options.width, options.height, false);
  renderer.setClearColor(brand.darkBackground);
  renderer.toneMapping = THREE.NoToneMapping;
  renderer.toneMappingExposure = 1.1;
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(brand.darkBackground);
  const camera = new THREE.PerspectiveCamera(33, options.width / options.height, 0.1, 80);
  const environment = new RoomEnvironment();
  const pmrem = new THREE.PMREMGenerator(renderer);
  const env = pmrem.fromScene(environment, 0.06);
  scene.environment = env.texture;
  scene.environmentIntensity = 0.55;
  environment.dispose(); pmrem.dispose();
  const key = new THREE.DirectionalLight(brand.sceneLight, 3); key.position.set(-3, 6, 9); scene.add(key);
  const rim = new THREE.DirectionalLight(brand.amber, 4); rim.position.set(2, 1, -5); scene.add(rim);
  scene.add(new THREE.AmbientLight(brand.paper, 0.3));
  const stack = new THREE.Group(); scene.add(stack);
  const loader = new THREE.TextureLoader();
  const assetBase = options.assetBase ?? '/scene/';
  const manifest = await fetch(`${assetBase}manifest.json`).then((response) => {
    if (!response.ok) throw new Error('Scene capture manifest unavailable');
    return response.json() as Promise<{ focus: { u: number; v: number } }>;
  }).catch((error) => { env.dispose(); renderer.dispose(); throw error; });
  let textures: THREE.Texture[];
  try { textures = await Promise.all(LAYERS.map((layer) => loader.loadAsync(`${assetBase}${layer.id}.webp`))); }
  catch (error) { env.dispose(); renderer.dispose(); throw error; }
  const anchors: THREE.Vector3[] = [];
  const glints: THREE.Line[] = [];
  const tracers: THREE.Line[] = [];
  const panels = LAYERS.map((_, index) => {
    const group = new THREE.Group(); stack.add(group);
    const plate = new THREE.Mesh(new RoundedBoxGeometry(W + 0.09, H + 0.09, 0.075, 3, 0.05), new THREE.MeshStandardMaterial({ color: index % 2 ? brand.sceneRaised : brand.sceneOLED, metalness: 0.25, roughness: 0.6 }));
    group.add(plate);
    const texture = textures[index]; texture.colorSpace = THREE.SRGBColorSpace; texture.anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
    const textureImage = texture.image as HTMLImageElement;
    const screenHeight = index === 0 ? H : Math.min(H, W * textureImage.height / textureImage.width);
    const screen = new THREE.Mesh(new THREE.PlaneGeometry(W, screenHeight), new THREE.MeshBasicMaterial({ map: texture, color: new THREE.Color(0.85, 0.85, 0.85), toneMapped: false }));
    screen.position.set(0, index === 0 ? 0 : -H / 2 + screenHeight / 2, 0.043); group.add(screen);
    const top = line([new THREE.Vector3(-W / 2, H / 2 + 0.02, 0.05), new THREE.Vector3(W / 2, H / 2 + 0.02, 0.05)], brand.sceneLight, 0.6); group.add(top);
    const edge = new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(-W / 2 - 0.025, -H / 2 - 0.025, 0.05), new THREE.Vector3(W / 2 + 0.025, -H / 2 - 0.025, 0.05),
      new THREE.Vector3(W / 2 + 0.025, H / 2 + 0.025, 0.05), new THREE.Vector3(-W / 2 - 0.025, H / 2 + 0.025, 0.05),
    ]), new THREE.LineBasicMaterial({ color: new THREE.Color(brand.amber).multiplyScalar(2), transparent: true, opacity: 0.45, toneMapped: false }));
    group.add(edge);
    // A soft shadow/glow underneath each opaque screen supplies separation without translucent plastic.
    const shadow = new THREE.Mesh(new THREE.PlaneGeometry(W + 1.2, H + 1.2), new THREE.ShaderMaterial({ transparent: true, depthWrite: false,
      uniforms: { tint: { value: new THREE.Color(brand.amber) }, strength: { value: 0.1 } },
      vertexShader: 'varying vec2 uv0; void main(){uv0=uv;gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.);}',
      fragmentShader: 'varying vec2 uv0; uniform vec3 tint; uniform float strength; void main(){vec2 q=abs(uv0-.5)*2.;float d=max(q.x-.72,q.y-.68);float a=exp(-max(0.,d)*22.)*(1.-smoothstep(.82,1.,max(q.x,q.y)));gl_FragColor=vec4(tint,a*strength);}',
    })); shadow.position.z = -0.12; group.add(shadow);
    const anchor = new THREE.Vector3(W / 2 + 1.3, -H / 2 + 0.28, 0.05); anchors.push(anchor);
    const tracer = line([new THREE.Vector3(W / 2 - 0.25, -H / 2 + 0.28, 0.06), anchor], brand.amber, 0.65); group.add(tracer); tracers.push(tracer);
    const glint = line([new THREE.Vector3(), new THREE.Vector3()], brand.sceneLight, 0.9); group.add(glint); glints.push(glint);
    return { group, edge, shadow };
  });
  // Continuous request path through the stack; short moving highlights, never packets or dots.
  const spine = line(Array.from({ length: 5 }, () => new THREE.Vector3()), brand.amber, 0.35); stack.add(spine);
  const composer = options.light ? null : new EffectComposer(renderer);
  composer?.addPass(new RenderPass(scene, camera));
  const dof = composer ? new BokehPass(scene, camera, { focus: 8, aperture: 0.00012, maxblur: 0.008 }) : null;
  const bloom = composer ? new UnrealBloomPass(new THREE.Vector2(options.width, options.height), 0.6, 0.25, 0.85) : null;
  if (composer && dof && bloom) { composer.addPass(dof); composer.addPass(bloom); }
  const grain = composer ? new ShaderPass({ uniforms: { tDiffuse: { value: null }, time: { value: 0 } },
    vertexShader: 'varying vec2 vUv; void main(){vUv=uv;gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.);}',
    fragmentShader: 'uniform sampler2D tDiffuse; uniform float time; varying vec2 vUv; void main(){vec4 c=texture2D(tDiffuse,vUv);float n=fract(sin(dot(vUv+time,vec2(12.9898,78.233)))*43758.5453)-.5;gl_FragColor=vec4(c.rgb+n*.004,c.a);}',
  }) : null;
  if (composer && grain) composer.addPass(grain);
  composer?.addPass(new OutputPass());
  const raycaster = new THREE.Raycaster();
  let width = options.width, height = options.height;
  return {
    render(frame: StudioFrame) {
      const intro = clamp(frame.intro ?? 1, 0, 1);
      const dolly = 1 - Math.pow(1 - intro, 4);
      const [px, py] = frame.pointer ?? [0, 0];
      stack.rotation.set(-0.65 + py * 0.06, 0.12 + px * 0.06 + Math.sin(frame.seconds * 0.13) * 0.015, -0.17);
      const hero = frame.framing === 'hero';
      stack.position.set(-0.4, hero ? THREE.MathUtils.lerp(-0.65, 0.1, frame.spread) : 0.1, 0);
      panels.forEach(({ group, edge, shadow }, index) => {
        const stagger = smooth(frame.spread * 1.24 - index * 0.06);
        group.position.z = (2 - index) * (0.09 + GAP * stagger);
        group.position.y = (2 - index) * 0.42 * stagger;
        edge.material.opacity = (frame.active ?? Math.floor(frame.seconds * 0.65) % 5) === index ? 0.95 : 0.23;
        (shadow.material as THREE.ShaderMaterial).uniforms.strength.value = options.light ? 0.025 : 0.065;
        tracers[index].visible = frame.spread > 0.25;
        const phase = (frame.seconds * 0.45 - index * 0.12 + 10) % 1;
        const start = new THREE.Vector3(-W / 2 + phase * (W - 0.4), -H / 2 - 0.025, 0.06);
        const end = start.clone().add(new THREE.Vector3(0.4, 0, 0));
        glints[index].geometry.setFromPoints([start, end]);
        glints[index].visible = frame.spread > 0.15;
      });
      spine.geometry.setFromPoints(panels.map(({ group }) => new THREE.Vector3(-W / 2 + 0.15, -H / 2 - 0.12, group.position.z)));
      spine.visible = frame.spread > 0.2;
      const baseDistance = width < height ? 23 : 18.8;
      const finalDistance = hero ? baseDistance * THREE.MathUtils.lerp(0.76, 1, frame.spread) : baseDistance;
      stack.updateMatrixWorld(true);
      const focus = panels[0].group.localToWorld(new THREE.Vector3((manifest.focus.u - 0.5) * W, (0.5 - manifest.focus.v) * H, 0.043));
      camera.position.set(THREE.MathUtils.lerp(focus.x, 0.4, dolly), THREE.MathUtils.lerp(focus.y, 0.3, dolly), THREE.MathUtils.lerp(focus.z + 2.07, finalDistance, dolly));
      camera.lookAt(THREE.MathUtils.lerp(focus.x, 0, dolly), THREE.MathUtils.lerp(focus.y, 0, dolly), THREE.MathUtils.lerp(focus.z, 0, dolly));
      if (dof) {
        dof.enabled = intro < 0.85;
        (dof.uniforms as { focus: { value: number } }).focus.value = camera.position.distanceTo(focus);
      }
      if (grain) grain.uniforms.time.value = Math.floor(frame.seconds * 24);
      scene.updateMatrixWorld(); camera.updateMatrixWorld();
      if (composer) composer.render(); else renderer.render(scene, camera);
      return panels.map(({ group }, index) => {
        const point = group.localToWorld(anchors[index].clone()).project(camera);
        return { x: (point.x + 1) * width / 2, y: (1 - point.y) * height / 2, opacity: smooth((frame.spread - 0.35) * 2) * dolly };
      });
    },
    hit(x: number, y: number) {
      raycaster.setFromCamera(new THREE.Vector2(x, y), camera);
      const hit = raycaster.intersectObjects(panels.map(({ group }) => group), true)[0];
      return hit ? panels.findIndex(({ group }) => hit.object.parent === group) : -1;
    },
    resize(w: number, h: number) { width = w; height = h; renderer.setSize(w, h, false); composer?.setSize(w, h); camera.aspect = w / h; camera.updateProjectionMatrix(); },
    dispose() {
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.Line) {
          object.geometry.dispose();
          for (const material of Array.isArray(object.material) ? object.material : [object.material]) material.dispose();
        }
      });
      textures.forEach((texture) => texture.dispose()); env.dispose();
      for (const pass of composer?.passes ?? []) pass.dispose(); composer?.dispose(); renderer.dispose();
    },
  };
}
