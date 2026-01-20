import init, { init_webgpu, render, resize, reset_camera } from '../../crates/physicall-core/pkg/physicall_core';

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

function renderLoop(): void {
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

  // Set up keyboard handler for camera reset
  document.addEventListener('keydown', (e) => {
    if (e.key === 'r' || e.key === 'R') {
      reset_camera();
    }
  });

  // Start render loop
  requestAnimationFrame(renderLoop);
}

main().catch((e) => {
  console.error(e);
  showError(`Unexpected error: ${e instanceof Error ? e.message : String(e)}`);
});
