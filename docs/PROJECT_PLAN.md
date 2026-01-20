# Physicall - Project Plan

**Rust + WebAssembly + WebGPU Physical Simulations**

---

## Vision & Goals

### Vision
A browser-based physical simulation framework built with Rust, compiled to WebAssembly, leveraging WebGPU for GPU-accelerated computation and rendering. This is a personal learning project to gain deep understanding of Rust, WebAssembly, WebGPU graphics programming, and physics simulation fundamentals.

### Core Goals
1. **Learn Rust** - Ownership, lifetimes, traits, and systems programming
2. **Learn WebGPU** - Modern GPU APIs, shaders (WGSL), compute pipelines
3. **Learn Physics Simulation** - Integration methods, collision detection, constraint solving
4. **Build Something Visual** - Interactive simulations running in the browser

### Non-Goals (Out of Scope)
- Multiplayer/networking capabilities
- Mobile device support
- Production-grade performance optimization
- Competing with established physics engines

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Browser (Chrome/Firefox/Safari)                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────┐     ┌─────────────────────────────────────────┐   │
│  │   TypeScript UI     │     │           WebAssembly Module            │   │
│  │   (Vite + HTML)     │────▶│              (Rust/wgpu)                │   │
│  │                     │     │                                         │   │
│  │  - Controls Panel   │     │  ┌─────────────┐  ┌─────────────────┐  │   │
│  │  - Parameter Sliders│     │  │  Renderer   │  │ Physics Engine  │  │   │
│  │  - Simulation Select│     │  │   (wgpu)    │  │  (GPU Compute)  │  │   │
│  │  - Stats Display    │     │  └──────┬──────┘  └────────┬────────┘  │   │
│  └─────────────────────┘     │         │                  │           │   │
│                              │         ▼                  ▼           │   │
│                              │  ┌─────────────────────────────────┐   │   │
│                              │  │      WebGPU Device/Queue        │   │   │
│                              │  │                                 │   │   │
│                              │  │  - Render Pipeline (vertex/frag)│   │   │
│                              │  │  - Compute Pipeline (physics)   │   │   │
│                              │  │  - GPU Buffers (positions, vel) │   │   │
│                              │  └─────────────────────────────────┘   │   │
│                              └─────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
                              ┌───────────────────┐
                              │   GPU Hardware    │
                              │  (via WebGPU API) │
                              └───────────────────┘
```

### Component Responsibilities

| Component | Responsibility |
|-----------|---------------|
| **TypeScript UI** | User controls, parameter adjustment, simulation selection |
| **WASM Module** | Core simulation logic, rendering orchestration |
| **Renderer (wgpu)** | Vertex/fragment shaders, drawing primitives, camera |
| **Physics Engine** | Compute shaders for simulation steps, collision detection |
| **GPU Buffers** | Position, velocity, force data stored on GPU |

---

## Technical Decisions

### Decided

| ID | Decision | Choice | Rationale |
|----|----------|--------|-----------|
| TD1 | Language | Rust | Learning goal, memory safety, WASM target |
| TD2 | WASM Build Tool | wasm-pack | Standard tooling, generates TS bindings |
| TD3 | Graphics API | wgpu | Cross-platform WebGPU, native fallback |
| TD4 | Shader Language | WGSL | Native WebGPU, no conversion needed |
| TD5 | Physics Location | GPU (Compute Shaders) | Learning goal, parallel simulation |
| TD6 | Frontend | TypeScript + Vite | Modern DX, fast rebuilds, familiar |
| TD7 | Renderer | Custom wgpu | Maximum learning, full control |
| TD8 | Primary Browser | Chrome | Best WebGPU support, devtools |

### Open (To Decide During Implementation)

| ID | Question | Options | Decide By |
|----|----------|---------|-----------|
| TD9 | Integration method | (A) Euler (B) Verlet (C) RK4 | M1 |
| TD10 | Collision broad phase | (A) Spatial hash (B) Grid (C) None initially | M2 |
| TD11 | Camera system | (A) Orbit (B) FPS-style (C) 2D pan/zoom | M1 |
| TD12 | State serialization | (A) JSON (B) Binary (C) None initially | M3 |

---

## Project Structure

```
physicall/
├── Cargo.toml                    # Workspace root
├── package.json                  # NPM workspace root
├── docs/
│   └── PROJECT_PLAN.md           # This document
│
├── crates/
│   └── physicall-core/           # Main Rust WASM library
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs            # WASM entry point, exports
│           ├── renderer/         # wgpu rendering code
│           │   ├── mod.rs
│           │   ├── pipeline.rs
│           │   └── camera.rs
│           ├── physics/          # Compute shader physics
│           │   ├── mod.rs
│           │   ├── particle.rs
│           │   └── constraints.rs
│           └── simulation/       # Simulation types
│               ├── mod.rs
│               ├── particles.rs
│               ├── nbody.rs
│               └── cloth.rs
│
├── shaders/                      # WGSL shader files
│   ├── render/
│   │   ├── particle.wgsl
│   │   └── mesh.wgsl
│   └── compute/
│       ├── integrate.wgsl
│       └── collision.wgsl
│
└── web/                          # TypeScript frontend
    ├── package.json
    ├── vite.config.ts
    ├── index.html
    ├── src/
    │   ├── main.ts               # Entry point
    │   ├── wasm-loader.ts        # WASM initialization
    │   ├── controls/             # UI components
    │   └── style.css
    └── public/
```

---

## Milestones

| ID | Milestone | Description | Dependencies | Status |
|----|-----------|-------------|--------------|--------|
| M0 | Foundation | Project setup, build pipeline, hello triangle | - | NOT_STARTED |
| M1 | Basic Renderer | Camera, primitives, instanced rendering | M0 | NOT_STARTED |
| M2 | Particle System | GPU compute particles, forces, integration | M1 | NOT_STARTED |
| M3 | Interactions | Mouse input, parameter controls, UI | M2 | NOT_STARTED |
| M4 | Advanced Physics | Collision detection, constraints, cloth | M3 | NOT_STARTED |
| M5 | Polish | Multiple simulations, presets, export | M4 | NOT_STARTED |

---

## Epics & User Stories

### M0: Foundation

#### E0.1: Project Setup

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E0.1.1 | As a developer, I can build the Rust crate to WASM | `wasm-pack build` succeeds | S |
| E0.1.2 | As a developer, I can run the web app locally | `npm run dev` serves app at localhost | S |
| E0.1.3 | As a developer, I can see WASM logs in browser console | `console_log!` macro works from Rust | S |

#### E0.2: WebGPU Initialization

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E0.2.1 | As a user, I see a colored canvas when WebGPU is available | Canvas renders solid color via wgpu | M |
| E0.2.2 | As a user, I see an error message if WebGPU is unsupported | Graceful fallback with instructions | S |
| E0.2.3 | As a developer, I can resize the canvas and it adapts | Surface reconfigures on window resize | M |

#### E0.3: Hello Triangle

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E0.3.1 | As a user, I see a triangle rendered on screen | Basic render pipeline working | M |
| E0.3.2 | As a developer, I can modify shader and see changes | Shader loading from .wgsl files | S |

---

### M1: Basic Renderer

#### E1.1: Camera System

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E1.1.1 | As a user, I can orbit the camera with mouse drag | Orbit camera with smooth rotation | M |
| E1.1.2 | As a user, I can zoom with scroll wheel | Camera distance adjusts | S |
| E1.1.3 | As a user, I can reset camera to default view | Reset button/hotkey works | S |

#### E1.2: Primitive Rendering

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E1.2.1 | As a developer, I can render points/particles | Point primitives with size | M |
| E1.2.2 | As a developer, I can render lines | Line primitives for debug/constraints | M |
| E1.2.3 | As a developer, I can render spheres | Instanced sphere mesh rendering | L |

#### E1.3: Instanced Rendering

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E1.3.1 | As a developer, I can render 10,000+ instances efficiently | Instanced draw calls working | M |
| E1.3.2 | As a developer, instance data comes from GPU buffer | No CPU-GPU transfer per frame | M |

---

### M2: Particle System

#### E2.1: GPU Compute Pipeline

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E2.1.1 | As a developer, I can run a compute shader | Basic compute dispatch works | M |
| E2.1.2 | As a developer, compute results are visible in render | Buffer shared between compute and render | M |

#### E2.2: Particle Simulation

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E2.2.1 | As a user, I see particles falling under gravity | Position integration in compute shader | M |
| E2.2.2 | As a user, particles bounce off floor boundary | Simple boundary collision | M |
| E2.2.3 | As a user, I can adjust gravity strength | Uniform buffer updated from JS | S |

#### E2.3: N-Body Simulation

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E2.3.1 | As a user, I see gravitational attraction between particles | N-body force calculation in compute | L |
| E2.3.2 | As a user, I can spawn particles at mouse position | Click-to-spawn interaction | M |

---

### M3: Interactions

#### E3.1: UI Controls

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E3.1.1 | As a user, I can pause/resume simulation | Play/pause toggle | S |
| E3.1.2 | As a user, I can reset simulation to initial state | Reset button clears and reinits | M |
| E3.1.3 | As a user, I can adjust simulation parameters via sliders | Sliders update uniforms in real-time | M |

#### E3.2: Mouse Interaction

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E3.2.1 | As a user, I can drag particles with mouse | Mouse ray picking, force application | L |
| E3.2.2 | As a user, I can spawn/delete particles | Context menu or keyboard shortcuts | M |

#### E3.3: Performance Display

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E3.3.1 | As a user, I see FPS counter | Frame time measurement displayed | S |
| E3.3.2 | As a user, I see particle count | Current simulation stats | S |

---

### M4: Advanced Physics

#### E4.1: Collision Detection

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E4.1.1 | As a user, particles collide with each other | Particle-particle collision response | L |
| E4.1.2 | As a developer, collision uses spatial hashing | O(n) broad phase, not O(n²) | L |

#### E4.2: Constraints

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E4.2.1 | As a user, I see distance constraints (springs) | Constraint solving in compute | L |
| E4.2.2 | As a user, I can see cloth simulation | Grid of particles with constraints | XL |

#### E4.3: Soft Bodies

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E4.3.1 | As a user, I can see a deformable object | Pressure-based soft body | XL |

---

### M5: Polish

#### E5.1: Multiple Simulations

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E5.1.1 | As a user, I can switch between simulation types | Dropdown selector works | M |
| E5.1.2 | As a user, each simulation has appropriate presets | Default values per simulation | M |

#### E5.2: Visual Polish

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E5.2.1 | As a user, I see velocity-based particle coloring | Color mapped to speed | M |
| E5.2.2 | As a user, I can toggle debug visualization | Constraint lines, grid overlay | M |

#### E5.3: Export/Share

| ID | Story | Acceptance Criteria | Size |
|----|-------|---------------------|------|
| E5.3.1 | As a user, I can save/load simulation state | Serialization to JSON | L |
| E5.3.2 | As a user, I can record simulation as video | Canvas capture API | L |

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| WebGPU browser compatibility issues | Medium | Medium | Target Chrome primarily, test Firefox/Safari periodically |
| WASM/JS interop complexity | Medium | Low | Start simple, use wasm-bindgen best practices |
| GPU compute shader debugging difficulty | High | Medium | Add CPU fallback for debugging, use validation layers |
| Scope creep (adding features) | Medium | Medium | Strict milestone boundaries, defer to M5+ |
| Rust learning curve slowing progress | Medium | Medium | Focus on one concept at a time, use Learn Wgpu tutorial |

---

## Learning Resources

### Primary Tutorials (Follow in Order)
1. **[Learn Wgpu](https://sotrh.github.io/learn-wgpu/)** - Comprehensive wgpu tutorial
2. **[WebGPU Fundamentals](https://webgpufundamentals.org/)** - WebGPU concepts (JS-based but applicable)
3. **[The Nature of Code](https://natureofcode.com/)** - Physics simulation fundamentals

### Reference Documentation
- [wgpu Documentation](https://docs.rs/wgpu/)
- [WGSL Specification](https://www.w3.org/TR/WGSL/)
- [wasm-bindgen Guide](https://rustwasm.github.io/docs/wasm-bindgen/)

### Example Projects
- [wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples)
- [par-particle-life](https://github.com/paulrobello/par-particle-life) - GPU particle simulation

---

## Next Steps

The project plan is complete. To start implementation:

1. **Pick milestone M0** to begin
2. **For each epic/story**, use:
   - `/research` - Investigate implementation approach
   - `/plan` - Create detailed implementation plan
   - `/implement` - Execute the implementation

Suggested starting point: **E0.1.1** - Set up the Rust WASM build pipeline.
