# physicall-core

Core WASM library for Physicall - handles WebGPU rendering and physics simulation.

## Overview

This crate compiles to WebAssembly and provides:

- WebGPU device/surface initialization
- Render pipelines for primitives (points, lines, spheres)
- Orbit camera with mouse input handling
- Depth buffer management

## Dependencies

- **wgpu** - WebGPU implementation
- **wasm-bindgen** - Rust/JS interop
- **glam** - Math library (vectors, matrices)
- **bytemuck** - Safe transmutation for GPU buffers
- **web-sys** - DOM bindings

## Building

```bash
# Build WASM package
wasm-pack build --target web

# With release optimizations
wasm-pack build --target web --release
```

Output goes to `pkg/` directory.

## API

The main entry point is `State::new()` which initializes WebGPU and returns a handle for rendering:

```typescript
import init, { State } from './pkg/physicall_core.js';

await init();
const state = await State.new("canvas-id");

// Render loop
function frame() {
  state.update();
  state.render();
  requestAnimationFrame(frame);
}
frame();
```

## Architecture

```
State
├── device/queue (wgpu)
├── surface/config
├── camera (OrbitCamera)
├── depth_texture
└── pipelines
    ├── line_pipeline
    ├── point_pipeline
    └── sphere_pipeline
```

## Shaders

Shaders are loaded from `../../shaders/`:

- `line.wgsl` - Simple position + color passthrough
- `point.wgsl` - Instanced quads with circular discard
- `sphere.wgsl` - Instanced icosphere with diffuse lighting
