import init, {
  init_webgpu,
  render,
  resize,
  reset_camera,
  set_paused,
  reset_simulation,
  reset_simulation_with_count,
  set_gravity,
  set_time_scale,
  get_particle_count,
} from '../../crates/physicall-core/pkg/physicall_core';

interface WebGPUCheckResult {
  supported: boolean;
  error?: string;
}

async function checkWebGPUSupport(): Promise<WebGPUCheckResult> {
  // Check secure context (HTTPS or localhost required)
  if (!window.isSecureContext) {
    return {
      supported: false,
      error: 'WebGPU requires a secure connection (HTTPS or localhost).',
    };
  }

  // Check if navigator.gpu exists
  if (!navigator.gpu) {
    return {
      supported: false,
      error: getBrowserSpecificError(),
    };
  }

  // Try to get an adapter to verify GPU is available
  try {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
      return {
        supported: false,
        error: 'No WebGPU adapter available. Your GPU may not be supported, or hardware acceleration may be disabled.',
      };
    }
    return { supported: true };
  } catch (e) {
    return {
      supported: false,
      error: `WebGPU initialization failed: ${e instanceof Error ? e.message : String(e)}`,
    };
  }
}

function getBrowserSpecificError(): string {
  const ua = navigator.userAgent.toLowerCase();

  if (ua.includes('firefox')) {
    return 'WebGPU is not enabled in Firefox. Please update to Firefox 141+ (Windows) or Firefox 145+ (macOS), or enable WebGPU in about:config.';
  }
  if (ua.includes('safari') && !ua.includes('chrome')) {
    return 'WebGPU requires Safari 26+ on macOS/iOS 26+. Please update your operating system and browser.';
  }
  if (ua.includes('chrome') || ua.includes('chromium') || ua.includes('edg')) {
    return 'WebGPU requires Chrome/Edge 113+. Please update your browser and ensure "Use graphics acceleration" is enabled in Settings.';
  }

  return 'Your browser does not support WebGPU. Please try Chrome 113+, Firefox 141+, or Safari 26+.';
}

function showError(message: string): void {
  const canvas = document.getElementById('canvas');
  const errorDiv = document.getElementById('error');

  if (canvas) {
    canvas.classList.add('hidden');
  }

  if (errorDiv) {
    errorDiv.innerHTML = `
      <h2>WebGPU Not Available</h2>
      <p>${message}</p>
      <p style="margin-top: 1rem; font-size: 0.9em; opacity: 0.8;">
        WebGPU is a modern graphics API. For the best experience, use the latest version of
        <a href="https://www.google.com/chrome/" target="_blank">Chrome</a> or
        <a href="https://www.microsoft.com/edge" target="_blank">Edge</a>.
      </p>
    `;
    errorDiv.classList.remove('hidden');
  }
}

const canvas = document.getElementById('canvas') as HTMLCanvasElement;

// Pending resize dimensions (applied in render loop)
let pendingWidth: number | null = null;
let pendingHeight: number | null = null;

// UI state
let isPaused = false;

function setupResizeObserver(): void {
  const observer = new ResizeObserver((entries) => {
    const entry = entries[0];
    if (!entry) return;

    // Cap devicePixelRatio at 2 for performance on high-DPI displays
    const dpr = Math.min(2, window.devicePixelRatio);

    // Use devicePixelContentBoxSize if available (more accurate for HiDPI)
    // Fall back to contentBoxSize * dpr
    const dpSize = entry.devicePixelContentBoxSize?.[0];
    if (dpSize) {
      pendingWidth = Math.round(dpSize.inlineSize);
      pendingHeight = Math.round(dpSize.blockSize);
    } else {
      const contentSize = entry.contentBoxSize[0];
      pendingWidth = Math.round(contentSize.inlineSize * dpr);
      pendingHeight = Math.round(contentSize.blockSize * dpr);
    }
  });

  // Try to observe with device-pixel-content-box for best HiDPI support
  try {
    observer.observe(canvas, { box: 'device-pixel-content-box' });
  } catch {
    // Fall back to content-box if device-pixel-content-box not supported
    observer.observe(canvas, { box: 'content-box' });
  }
}

function setupUIControls(): void {
  const pauseBtn = document.getElementById('pause-btn') as HTMLButtonElement;
  const resetBtn = document.getElementById('reset-btn') as HTMLButtonElement;
  const particleCountInput = document.getElementById('particle-count-input') as HTMLInputElement;
  const gravitySlider = document.getElementById('gravity-slider') as HTMLInputElement;
  const gravityValue = document.getElementById('gravity-value') as HTMLSpanElement;
  const speedSlider = document.getElementById('speed-slider') as HTMLInputElement;
  const speedValue = document.getElementById('speed-value') as HTMLSpanElement;
  const pausedOverlay = document.getElementById('paused-overlay') as HTMLDivElement;

  // Pause/Resume button
  pauseBtn.addEventListener('click', () => {
    isPaused = !isPaused;
    set_paused(isPaused);
    pauseBtn.textContent = isPaused ? 'Resume' : 'Pause';
    if (isPaused) {
      pausedOverlay.classList.remove('hidden');
    } else {
      pausedOverlay.classList.add('hidden');
    }
  });

  // Reset button — uses particle count from input
  resetBtn.addEventListener('click', () => {
    const count = parseInt(particleCountInput.value, 10);
    if (count > 0) {
      reset_simulation_with_count(count);
    } else {
      reset_simulation();
    }
  });

  // Gravity slider
  gravitySlider.addEventListener('input', () => {
    const value = parseFloat(gravitySlider.value);
    set_gravity(value);
    gravityValue.textContent = value.toFixed(1);
  });

  // Speed slider
  speedSlider.addEventListener('input', () => {
    const value = parseFloat(speedSlider.value);
    set_time_scale(value);
    speedValue.textContent = value.toFixed(1);
  });
}

// Stats display state
let lastFrameTimestamp = 0;
let smoothedFps = 0;
let smoothedFrameTime = 0;
let lastDisplayUpdate = 0;
let fpsValue: HTMLSpanElement | null = null;
let frametimeValue: HTMLSpanElement | null = null;
let statsParticleCount: HTMLSpanElement | null = null;

const EMA_ALPHA = 0.1;
const DISPLAY_INTERVAL = 500; // ms

function renderLoop(timestamp: DOMHighResTimeStamp): void {
  // Apply pending resize before rendering
  if (pendingWidth !== null && pendingHeight !== null) {
    if (pendingWidth > 0 && pendingHeight > 0) {
      canvas.width = pendingWidth;
      canvas.height = pendingHeight;
      resize(pendingWidth, pendingHeight);
    }
    pendingWidth = null;
    pendingHeight = null;
  }

  render();

  // FPS / frame time tracking
  if (lastFrameTimestamp > 0) {
    const frameTime = timestamp - lastFrameTimestamp;
    const fps = 1000 / frameTime;
    smoothedFps = smoothedFps === 0 ? fps : smoothedFps * (1 - EMA_ALPHA) + fps * EMA_ALPHA;
    smoothedFrameTime = smoothedFrameTime === 0 ? frameTime : smoothedFrameTime * (1 - EMA_ALPHA) + frameTime * EMA_ALPHA;
  }
  lastFrameTimestamp = timestamp;

  // Throttled DOM update every 500ms
  if (timestamp - lastDisplayUpdate >= DISPLAY_INTERVAL) {
    lastDisplayUpdate = timestamp;
    if (fpsValue) fpsValue.textContent = smoothedFps > 0 ? String(Math.round(smoothedFps)) : '--';
    if (frametimeValue) frametimeValue.textContent = smoothedFrameTime > 0 ? smoothedFrameTime.toFixed(1) : '--';
    if (statsParticleCount) statsParticleCount.textContent = String(get_particle_count());
  }

  requestAnimationFrame(renderLoop);
}

async function main() {
  // Check WebGPU support before doing anything
  const check = await checkWebGPUSupport();
  if (!check.supported) {
    showError(check.error ?? 'WebGPU is not supported.');
    return;
  }

  // Initialize WASM
  await init();
  console.log('WASM loaded successfully');

  // Set initial canvas size to match display size
  const dpr = Math.min(2, window.devicePixelRatio);
  canvas.width = Math.round(canvas.clientWidth * dpr);
  canvas.height = Math.round(canvas.clientHeight * dpr);

  // Initialize WebGPU
  await init_webgpu('canvas');

  // Set up resize observer for responsive canvas
  setupResizeObserver();

  // Set up UI controls
  setupUIControls();

  // Set up stats display references
  fpsValue = document.getElementById('fps-value') as HTMLSpanElement;
  frametimeValue = document.getElementById('frametime-value') as HTMLSpanElement;
  statsParticleCount = document.getElementById('stats-particle-count') as HTMLSpanElement;

  // Set up keyboard handlers
  const statsPanel = document.getElementById('stats') as HTMLDivElement;
  let statsVisible = true;

  document.addEventListener('keydown', (e) => {
    if (e.key === 'r' || e.key === 'R') {
      reset_camera();
    }
    if (e.ctrlKey && (e.key === 'y' || e.key === 'Y')) {
      e.preventDefault();
      statsVisible = !statsVisible;
      statsPanel.classList.toggle('hidden', !statsVisible);
    }
  });

  // Start render loop
  requestAnimationFrame(renderLoop);
}

main().catch((e) => {
  console.error(e);
  showError(`Unexpected error: ${e instanceof Error ? e.message : String(e)}`);
});
