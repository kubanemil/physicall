# Physicall

**Rust + WebAssembly + WebGPU Physical Simulations**

A browser-based physical simulation framework built with Rust, compiled to WebAssembly, leveraging WebGPU for GPU-accelerated computation and rendering.

## Features

- WebGPU-based rendering with wgpu
- Orbit camera with mouse controls (drag to rotate, scroll to zoom, shift+drag to pan)
- Primitive rendering: points, lines, spheres
- Instanced rendering for efficient particle systems
- Depth buffer for correct 3D ordering

## Project Structure

```
physicall/
├── crates/
│   └── physicall-core/     # Rust WASM library (rendering, physics)
├── web/                    # TypeScript frontend (Vite)
├── shaders/                # WGSL shaders
│   ├── line.wgsl          # Line rendering
│   ├── point.wgsl         # Point/particle rendering
│   └── sphere.wgsl        # Sphere rendering with lighting
└── docs/                   # Documentation and plans
```

## Prerequisites

- [Rust](https://rustup.rs/) (with `wasm32-unknown-unknown` target)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (v18+)

## Quick Start

```bash
# Install Rust WASM target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack

# Build the WASM module
wasm-pack build crates/physicall-core --target web

# Install web dependencies
cd web && npm install

# Start development server
npm run dev
```

Open http://localhost:5173 in a WebGPU-compatible browser (Chrome 113+, Firefox 121+, Safari 18+).

## Controls

- **Left-click + drag**: Orbit camera
- **Scroll wheel**: Zoom in/out
- **Shift + left-click + drag**: Pan camera
- **R key**: Reset camera to default position

## Development

### Build Commands

```bash
# Build WASM (from project root)
wasm-pack build crates/physicall-core --target web

# Run clippy
cargo clippy --target wasm32-unknown-unknown -p physicall-core

# Build web frontend
cd web && npm run build

# Development server with hot reload
cd web && npm run dev
```

## Tech Stack

- **Rust** - Systems programming language
- **wgpu** - Safe Rust WebGPU implementation
- **wasm-bindgen** - Rust/JavaScript interop
- **TypeScript** - Frontend type safety
- **Vite** - Fast frontend build tool

## License

MIT
