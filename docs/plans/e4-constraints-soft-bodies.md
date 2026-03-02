---
date: 2026-03-01
author: claude-opus-4-6
status: complete
branch: main
related_research: docs/prfaq/e4-constraints-soft-bodies.md
---

# Plan: E4 - Constraints & Soft Bodies

## Overview

Add distance constraints, cloth simulation, rope, and pressure-based soft bodies to the Physicall simulation. Currently particles only interact through N-body gravity and collision — they can't form connected structures. After this implementation, users can switch between simulation presets (N-body, Cloth, Rope, Soft Body) and interact with spring-connected particle structures that stretch, deform, and restore their shape.

The implementation is split into four phases: (1) GPU constraint infrastructure, (2) simulation modes with cloth and rope presets, (3) soft body with pressure, (4) final verification and tuning.

## Architecture Decisions

- **Thread-per-particle constraint solver**: Each GPU thread handles one particle and iterates over its own constraints. No atomic operations needed — each thread writes only to its own particle. This is Gauss-Seidel-like on the GPU (nondeterministic read ordering), which actually converges faster than Jacobi for PBD.
- **Per-particle constraint lookup**: Fixed-bucket index structure (like spatial hash). Each particle stores up to `MAX_CONSTRAINTS_PER_PARTICLE = 12` constraint indices. Built on CPU during preset init, uploaded once to GPU.
- **Separate constraint pipeline layout**: New bind group layout for constraint solving with `[particle_bind_group, constraint_data_bind_group]`. Keeps constraint infrastructure decoupled from existing N-body pipeline.
- **In-place constraint solving**: Solver reads and writes the same particle buffer (the output of the physics pass). Each iteration is a separate compute dispatch with implicit barrier. 4 iterations per frame by default.
- **Mode-conditional physics shader**: Physics shader checks `simulation_mode` — for N-body, runs existing O(n²) gravity+collision loop. For constraint presets, applies simple downward gravity only (skips O(n²) entirely). Floor bounce always runs.
- **Reuse Particle._padding as flags**: Rename `_padding: f32` to `flags: u32`. Bit 0 = is_pinned. Zero change to buffer layout or vertex attributes (the field isn't read by render shaders). Pinned particles skip position updates in the solver.
- **Preset selector triggers reset**: Changing simulation mode reinitializes particles and constraints for the selected preset. No mid-simulation mode switching.
- **2D soft body only**: Soft body is a ring of particles (2D cross-section) with pressure computed via shoelace formula for enclosed area. 3D volumetric soft bodies deferred.
- **No self-collision for constraint presets**: The O(n²) collision loop is skipped for constraint modes. Cloth can fold through itself in V1. Constraints handle separation between connected particles.

## Prerequisites

- [x] E4.1 Collision detection with spatial hashing (current)
- [x] GPU compute pipeline, particle buffers, double-buffering (current)
- [x] UI controls infrastructure — sliders, buttons (current)
- [x] Line rendering pipeline (current, used for axes)

---

## Phase 1: Constraint Infrastructure

### Goal

Add GPU buffers, bind groups, compute shader, and pipeline for constraint solving. Extend SimParams and Particle struct. Wire up solver dispatches in the render loop. No presets or UI yet — infrastructure only, verifiable by building successfully.

### Files to Modify

- `crates/physicall-core/src/particle.rs` — Rename `_padding` → `flags`, add `Constraint` struct, add constraint constants, add `MAX_CONSTRAINTS` / `MAX_CONSTRAINTS_PER_PARTICLE`
- `crates/physicall-core/src/state.rs` — Add constraint buffers, constraint bind group layout/pipeline, constraint bind groups, solver dispatch loop in `render()`
- `shaders/particle_compute.wgsl` — Add `simulation_mode` branch: skip N-body loop for constraint modes, use simple gravity

### Files to Create

- `shaders/constraint_solve.wgsl` — Constraint solver compute shader

### Module Design

**`particle.rs` additions:**

New struct:
- `Constraint { index_a: u32, index_b: u32, rest_length: f32, _padding: u32 }` — 16 bytes, Pod+Zeroable

New constants:
- `MAX_CONSTRAINTS: u32 = 8192`
- `MAX_CONSTRAINTS_PER_PARTICLE: u32 = 12`
- `SOLVER_ITERATIONS: u32 = 4`
- `DEFAULT_STIFFNESS: f32 = 1.0`
- `DEFAULT_PRESSURE: f32 = 1.0`
- `DEFAULT_GRAVITY_Y: f32 = -9.8`

Particle struct change:
- `_padding: f32` → `flags: u32` (bit 0 = is_pinned). Initialize to `0u32`. Rust code uses `0.0f32` currently — change to `flags: 0` with type `u32`.

SimParams expansion (append after existing `_padding`):
- `constraint_count: u32`
- `stiffness: f32`
- `pressure: f32`
- `simulation_mode: u32` (0=NBody, 1=Cloth, 2=Rope, 3=SoftBody)
- `gravity_y: f32`
- `pin_count: u32`
- `_padding2: [f32; 2]` (pad to 96 bytes total, 16-byte aligned)

**`state.rs` additions:**

New buffers:
- `constraint_buffer: wgpu::Buffer` — `MAX_CONSTRAINTS * 16` bytes, STORAGE | COPY_DST
- `particle_constraint_indices_buffer: wgpu::Buffer` — `MAX_PARTICLES * MAX_CONSTRAINTS_PER_PARTICLE * 4` bytes, STORAGE | COPY_DST
- `particle_constraint_count_buffer: wgpu::Buffer` — `MAX_PARTICLES * 4` bytes, STORAGE | COPY_DST

New bind group layout (constraint_bind_group_layout):
- Group 0: Binding 0 — particles (storage, read_write)
- Group 0: Binding 1 — sim_params (uniform)
- Group 1: Binding 0 — constraints (storage, read_only)
- Group 1: Binding 1 — particle_constraint_indices (storage, read_only)
- Group 1: Binding 2 — particle_constraint_count (storage, read_only)

New bind groups:
- `constraint_particle_bind_groups: [wgpu::BindGroup; 2]` — one per particle buffer (for solving on whichever buffer physics wrote to)
- `constraint_data_bind_group: wgpu::BindGroup` — single instance

New pipeline:
- `constraint_solve_pipeline: wgpu::ComputePipeline` — uses constraint pipeline layout

New state fields:
- `simulation_mode: u32` (default 0 = NBody)
- `stiffness: f32` (default 1.0)
- `pressure: f32` (default 1.0)
- `active_constraint_count: u32` (default 0)

Dispatch changes in `render()` — after existing physics pass, if `active_constraint_count > 0`:
```
for iteration in 0..SOLVER_ITERATIONS:
    begin_compute_pass:
        set_pipeline(constraint_solve_pipeline)
        set_bind_group(0, constraint_particle_bind_groups[write_idx])
        set_bind_group(1, constraint_data_bind_group)
        dispatch_workgroups(ceil(active_particle_count / 256), 1, 1)
```

**`constraint_solve.wgsl`:**

Thread per particle. Reads own constraint indices from lookup buffers. For each constraint:
1. Find other particle index
2. Read other particle's position from same buffer
3. Compute distance delta: `(current_dist - rest_length) / current_dist * stiffness * 0.5`
4. Accumulate position correction
After all constraints, apply averaged correction. If pinned (flags & 1), skip position update.

**`particle_compute.wgsl` changes:**

Add simulation_mode to SimParams struct. Add branch:
```wgsl
if (params.simulation_mode == 0u) {
    // Existing N-body + collision loop (unchanged)
} else {
    // Simple downward gravity
    acceleration = vec3f(0.0, params.gravity_y, 0.0);
}
```

### Integration

- SimParams layout change affects `particle_compute.wgsl`, `point.wgsl`, `hash_insert.wgsl`, and new `constraint_solve.wgsl`. All must declare identical struct layout.
- Particle struct change (`_padding` → `flags`) affects `particle_compute.wgsl` and `point.wgsl` WGSL struct declarations.
- Existing N-body mode is completely unchanged when `simulation_mode == 0` and `active_constraint_count == 0`.

### Success Criteria

**Automated Verification:**
- [x] `cd crates/physicall-core && wasm-pack build --target web --out-dir pkg` — builds
- [x] `cd web && npm run build` — builds

**Manual Verification:**
- [ ] Existing N-body simulation works identically (no regression)
- [ ] No GPU validation errors in browser console

---

## Phase 2: Simulation Modes + Cloth & Rope Presets

### Goal

Visible cloth and rope. Users can switch presets and interact with constraint-based structures. Add cloth grid initialization, rope chain initialization, pin support, preset selector dropdown, and stiffness slider.

### Files to Modify

- `crates/physicall-core/src/particle.rs` — Add `create_cloth_particles()`, `create_rope_particles()` functions that return `(Vec<Particle>, Vec<Constraint>, Vec<Vec<u32>>)`
- `crates/physicall-core/src/state.rs` — Add `set_simulation_mode()` method that reinitializes particles + constraints, update `reset_simulation()` to respect current mode
- `crates/physicall-core/src/lib.rs` — Add WASM exports: `set_simulation_mode(mode: u32)`, `set_stiffness(value: f32)`, `get_simulation_mode() -> u32`
- `web/src/main.ts` — Add preset selector dropdown, stiffness slider, import new WASM functions
- `web/index.html` — Add preset selector and stiffness slider DOM elements

### Module Design

**`particle.rs` — Cloth initialization:**

`create_cloth_particles(cols: u32, rows: u32, spacing: f32) -> (Vec<Particle>, Vec<Constraint>, Vec<Vec<u32>>)`

- Creates `cols × rows` particle grid in XY plane (centered at origin, hanging down)
- Particles at top-left and top-right corners have `flags = 1` (pinned)
- Structural constraints: horizontal + vertical neighbors (rest_length = spacing)
- Shear constraints: diagonal neighbors (rest_length = spacing × √2)
- Returns particles, flat constraint array, and per-particle constraint index lists
- Default: 30×30 grid = 900 particles, spacing = 0.08, ~3400 constraints

**`particle.rs` — Rope initialization:**

`create_rope_particles(count: u32, spacing: f32) -> (Vec<Particle>, Vec<Constraint>, Vec<Vec<u32>>)`

- Creates linear chain of particles along X axis
- First particle pinned (flags = 1)
- Distance constraints between consecutive particles
- Default: 50 particles, spacing = 0.06, 49 constraints

**`state.rs` — Mode switching:**

`set_simulation_mode(&mut self, mode: u32)`:
1. Set `self.simulation_mode = mode`
2. Call appropriate init function (N-body ring, cloth grid, rope chain)
3. Upload particles to both particle buffers
4. Upload constraints, constraint indices, constraint counts to GPU buffers
5. Update `active_particle_count` and `active_constraint_count`
6. Reset `frame_index` to 0

**`state.rs` — SimParams population:**

In `render()`, add to SimParams construction:
- `constraint_count: self.active_constraint_count`
- `stiffness: self.stiffness`
- `pressure: self.pressure`
- `simulation_mode: self.simulation_mode`
- `gravity_y: DEFAULT_GRAVITY_Y`

**UI additions (`main.ts` + `index.html`):**

Preset selector: `<select>` dropdown with options: N-Body, Cloth, Rope. On change, calls `set_simulation_mode(value)`.

Stiffness slider: range 0.0–1.0, step 0.05, default 1.0. On input, calls `set_stiffness(value)`. Visible only when simulation_mode != 0.

### Integration

- Preset selector change triggers full reinitialization (like reset). Camera stays in place.
- Stiffness flows through SimParams to constraint_solve.wgsl each frame.
- Cloth particles start in a flat grid, gravity pulls them down, top corners stay pinned.
- Rope particles start horizontal, gravity pulls the chain into a catenary curve.
- Drag interaction works: dragging a cloth/rope particle pulls neighbors via constraints.
- Existing sliders (gravity, time scale) work with constraint presets. Gravity slider maps to `gravity_y` in constraint mode.

### Success Criteria

**Automated Verification:**
- [x] `cd crates/physicall-core && wasm-pack build --target web --out-dir pkg` — builds
- [x] `cd web && npm run build` — builds

**Manual Verification:**
- [ ] Selecting "Cloth" shows a grid of particles that drapes under gravity
- [ ] Top corners of cloth stay pinned in place
- [ ] Dragging a cloth particle deforms the sheet, neighbors follow
- [ ] Selecting "Rope" shows a chain that swings from its pinned end
- [ ] Stiffness slider makes cloth stretchy (low) or rigid (high)
- [ ] Selecting "N-Body" returns to the original simulation
- [ ] Pause, reset, camera orbit, stats display all work in every mode
- [ ] Cloth (900 particles) runs at >30 FPS

---

## Phase 3: Soft Body + Pressure

### Goal

Add soft body preset — a ring of particles with internal pressure that deforms on impact and restores its shape. Add pressure computation and pressure slider.

### Files to Modify

- `crates/physicall-core/src/particle.rs` — Add `create_soft_body_particles()`, add `rest_area` constant
- `crates/physicall-core/src/state.rs` — Add pressure pass dispatch, add `rest_area` field, handle SoftBody mode in `set_simulation_mode()`
- `crates/physicall-core/src/lib.rs` — Add WASM export: `set_pressure(value: f32)`
- `web/src/main.ts` — Add pressure slider, add "Soft Body" option to preset selector
- `web/index.html` — Add pressure slider DOM element

### Files to Create

- `shaders/pressure.wgsl` — Pressure computation and force application shader

### Module Design

**`particle.rs` — Soft body initialization:**

`create_soft_body_particles(num_ring: u32, radius: f32) -> (Vec<Particle>, Vec<Constraint>, Vec<Vec<u32>>, f32)`

- Creates ring of `num_ring` particles (circle in XY plane, centered at origin)
- Perimeter constraints: each particle connected to next (rest_length = perimeter / num_ring)
- Cross-bracing: connect particle `i` to particle `i + num_ring/2` (diameter constraints for structural stability)
- Returns particles, constraints, per-particle indices, and rest_area (calculated via shoelace)
- Default: 40 ring particles, radius = 0.8

**`pressure.wgsl`:**

Thread per ring particle. For each edge (particle[i] → particle[i+1]):
1. Compute edge vector and outward normal
2. Compute edge length

After all edges, compute enclosed area (shoelace formula, via workgroup reduction or sequential sum — since ring is small, sequential is fine).

Pressure force per particle: `outward_normal * edge_length * (rest_area - current_area) * pressure_coefficient`

Apply force as velocity delta.

Note: This shader needs to know the ring ordering. Store ring indices in the constraint buffer or a separate small buffer. Since the ring is contiguous particles 0..num_ring, the shader can iterate by particle index directly.

**`state.rs` additions:**

New state field:
- `rest_area: f32` (computed during soft body init)

Dispatch changes — after constraint solver iterations, if `simulation_mode == 3` (SoftBody):
```
begin_compute_pass:
    set_pipeline(pressure_pipeline)
    set_bind_group(0, constraint_particle_bind_groups[write_idx])
    set_bind_group(1, ...) // pressure params
    dispatch_workgroups(ceil(active_particle_count / 256), 1, 1)
```

Pass `rest_area` through SimParams (repurpose one of the `_padding2` slots or extend further).

**UI additions:**

Pressure slider: range 0.0–5.0, step 0.1, default 1.0. Visible only when simulation_mode == 3 (SoftBody). On input, calls `set_pressure(value)`.

Add "Soft Body" as fourth option in preset selector dropdown.

### Integration

- Pressure pass runs after constraint solving, before render.
- Soft body starts as a circle, drops under gravity, squishes on floor contact, bounces back.
- Dragging deforms the ring; constraints + pressure restore shape.
- Pressure slider: 0 = no pressure (ring collapses), high = very bouncy.
- Cross-bracing prevents the ring from inverting (turning inside-out).

### Success Criteria

**Automated Verification:**
- [x] `cd crates/physicall-core && wasm-pack build --target web --out-dir pkg` — builds
- [x] `cd web && npm run build` — builds

**Manual Verification:**
- [ ] Selecting "Soft Body" shows a circular ring of particles
- [ ] Ring falls under gravity and deforms on floor impact
- [ ] Ring bounces back toward circular shape (pressure restores)
- [ ] Dragging the ring deforms it, releasing lets it restore
- [ ] Pressure slider changes how strongly the shape restores
- [ ] Low pressure: ring is floppy, collapses easily
- [ ] High pressure: ring is bouncy, snaps back quickly
- [ ] All other presets (N-Body, Cloth, Rope) still work correctly

---

## Phase 4: Final Verification

### Goal

Ensure all presets work together, all controls are functional, performance targets are met, and edge cases are handled.

### Success Criteria

**Automated Verification:**

Run full project verification per AGENTS.md:
```bash
cd crates/physicall-core && wasm-pack build --target web --out-dir pkg
cd web && npm install
cd web && npm run build
```

**Manual Verification:**

- [ ] All 4 presets selectable and visually correct: N-Body, Cloth, Rope, Soft Body
- [ ] Switching presets reinitializes correctly (no leftover state from previous preset)
- [ ] Cloth (900 particles, ~3400 constraints, 4 solver iterations) runs at >30 FPS
- [ ] Soft body deforms on impact and restores shape
- [ ] Rope swings naturally from pinned end
- [ ] Drag interaction works on all presets — pulling a particle moves connected neighbors
- [ ] Stiffness slider works for Cloth and Rope (stretchy ↔ rigid)
- [ ] Pressure slider works for Soft Body (floppy ↔ bouncy)
- [ ] Gravity slider maps to downward gravity for constraint presets
- [ ] Time scale slider works for all presets
- [ ] Pause/resume works for all presets
- [ ] Reset restores current preset's initial state
- [ ] Camera orbit, zoom, reset all work
- [ ] Stats display shows correct particle count per preset
- [ ] No GPU validation errors in console
- [ ] No energy gain or explosive instability in any preset
- [ ] Deleting a cloth particle creates a visible hole (broken constraints skipped)

---

## Rollback Plan

Each phase is independently verifiable. Phase 1 is pure infrastructure with no user-visible changes — if it causes issues, revert the commit. Phase 2 adds visible presets but the N-body mode is unchanged (simulation_mode == 0 skips all constraint code). Phase 3 adds soft body incrementally on top of working constraints.

If constraint solving causes instability, reduce `SOLVER_ITERATIONS` to 2 or lower stiffness default. If performance is insufficient, reduce default cloth grid from 30×30 to 20×20.

## Open Questions

- Should constraint lines be rendered (visual springs between connected particles)? Nice for debugging but adds rendering complexity. Could defer to a follow-up.
- Should the gravity slider behave differently in constraint mode (map to gravity_y magnitude) vs N-body mode (map to gravitational_constant)?
- What default cloth grid size balances visual quality with performance? 30×30 (900 particles) is the starting assumption.
- Should solver iterations be exposed as a slider or kept as a constant?
