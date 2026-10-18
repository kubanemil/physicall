import init, { App, scene_catalog } from '../../crates/physicall-web/pkg/physicall_web.js';
import { buildParams, formatNumber, type SceneSpec } from './ui';

const canvas = document.getElementById('canvas') as HTMLCanvasElement;
const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function fail(title: string, message: string): void {
  byId('error-title').textContent = title;
  byId('error-message').textContent = message;
  byId('error').hidden = false;
  for (const id of ['panel', 'readout']) byId(id).hidden = true;
  document.querySelector('.hint')?.remove();
  canvas.hidden = true;
}

async function webgpuProblem(): Promise<string | null> {
  if (!window.isSecureContext) return 'WebGPU only works on secure pages (HTTPS or localhost).';
  if (!('gpu' in navigator)) return 'This browser does not support WebGPU (navigator.gpu is missing).';
  try {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) return 'WebGPU is present, but no GPU adapter is available. Hardware acceleration may be off.';
  } catch (e) {
    return `Requesting a GPU adapter failed: ${errorText(e)}`;
  }
  return null;
}

function devicePixels(): [number, number] {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  return [Math.max(1, Math.round(canvas.clientWidth * dpr)), Math.max(1, Math.round(canvas.clientHeight * dpr))];
}

function canvasPoint(e: MouseEvent): [number, number] {
  const r = canvas.getBoundingClientRect();
  return [((e.clientX - r.left) * canvas.width) / r.width, ((e.clientY - r.top) * canvas.height) / r.height];
}

async function main(): Promise<void> {
  const problem = await webgpuProblem();
  if (problem) return fail('WebGPU is not available', problem);

  await init();
  const scenes: SceneSpec[] = JSON.parse(scene_catalog());
  const initial = scenes.find((s) => `#${s.id}` === location.hash) ?? scenes.find((s) => s.id === 'cloth') ?? scenes[0];

  [canvas.width, canvas.height] = devicePixels();
  let app: App;
  try {
    app = await App.create(canvas, initial.id);
  } catch (e) {
    return fail('Could not start the GPU simulation', errorText(e));
  }

  new ResizeObserver(() => {
    [canvas.width, canvas.height] = devicePixels();
    app.resize(canvas.width, canvas.height);
  }).observe(canvas);

  let current = initial;
  const sceneButtons = scenes.map((scene) => {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = scene.label;
    button.addEventListener('click', () => select(scene));
    return button;
  });
  byId('scenes').replaceChildren(...sceneButtons);

  const setParam = (key: string, value: number) => app.set_param(key, value);
  function select(scene: SceneSpec): void {
    if (scene !== current) app.load_scene(scene.id);
    current = scene;
    history.replaceState(null, '', `#${scene.id}`);
    sceneButtons.forEach((b, i) => b.setAttribute('aria-pressed', String(scenes[i] === scene)));
    buildParams(byId('params'), scene, setParam);
    byId('r-note').textContent =
      scene.id === 'nbody'
        ? 'G = 1 units. Leapfrog keeps the energy error bounded instead of drifting.'
        : 'SI units. Drag, friction and inelastic contacts remove energy; pins and the floor exchange momentum.';
  }
  select(initial);

  let paused = false;
  const pauseButton = byId<HTMLButtonElement>('pause');
  const togglePause = () => {
    paused = !paused;
    app.set_paused(paused);
    pauseButton.textContent = paused ? 'Resume' : 'Pause';
  };
  pauseButton.addEventListener('click', togglePause);
  byId('reset').addEventListener('click', () => app.reset());
  byId('camera').addEventListener('click', () => app.reset_camera());

  const speed = byId<HTMLInputElement>('speed');
  speed.addEventListener('input', () => {
    app.set_speed(Number(speed.value));
    byId('speed-value').textContent = `${Number(speed.value).toFixed(2)}×`;
  });

  canvas.addEventListener('pointerdown', (e) => {
    canvas.setPointerCapture(e.pointerId);
    app.pointer_down(...canvasPoint(e), e.button, e.shiftKey);
  });
  canvas.addEventListener('pointermove', (e) => app.pointer_move(...canvasPoint(e)));
  for (const type of ['pointerup', 'pointercancel'] as const) canvas.addEventListener(type, () => app.pointer_up());
  canvas.addEventListener('pointerleave', () => app.pointer_leave());
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  canvas.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      app.wheel(e.deltaY * (e.deltaMode === WheelEvent.DOM_DELTA_LINE ? 33 : 1));
    },
    { passive: false },
  );
  window.addEventListener('keydown', (e) => {
    if (e.target instanceof HTMLInputElement && e.key !== ' ') return;
    if (e.key === ' ') {
      e.preventDefault();
      togglePause();
    } else if (e.key === 'r' || e.key === 'R') {
      app.reset();
    }
  });

  const readout = (id: string, text: string) => (byId(`r-${id}`).textContent = text);
  let last = performance.now();
  let frameTime = 1 / 60;
  let lastReadout = 0;
  const tick = (now: number) => {
    const elapsed = Math.min((now - last) / 1000, 0.25);
    last = now;
    frameTime += 0.05 * (elapsed - frameTime);
    try {
      app.frame(elapsed);
    } catch (e) {
      return fail('The simulation stopped', errorText(e));
    }
    if (now - lastReadout > 250) {
      lastReadout = now;
      const s = app.stats();
      const si = current.id !== 'nbody';
      readout('time', `${s.time.toFixed(2)} s`);
      readout('particles', String(s.particles || '–'));
      readout('steps', String(s.steps));
      readout('fps', (1 / frameTime).toFixed(0));
      readout('kinetic', formatNumber(s.kinetic) + (si ? ' J' : ''));
      readout('potential', formatNumber(s.potential) + (si ? ' J' : ''));
      readout('total', formatNumber(s.total) + (si ? ' J' : ''));
      readout('drift', s.particles ? `${(100 * s.drift).toFixed(3)} %` : '–');
      readout('momentum', formatNumber(s.momentum) + (si ? ' kg·m/s' : ''));
      s.free();
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

main().catch((e) => fail('Something went wrong', errorText(e)));
