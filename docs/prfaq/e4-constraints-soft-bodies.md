# Constraints & Soft Bodies - PR FAQ

> **Reference**: E4 (E4.2 + E4.3)
> **Created**: 2026-03-01
> **Status**: Draft

---

## Press Release

**For** developers and physics enthusiasts using Physicall
**Who** want particles to form connected structures — ropes, cloth, and deformable shapes — not just free-floating particles
**The** Constraints & Soft Bodies system
**Is a** new capability in the physics simulation
**That** lets users create and interact with spring-connected particle structures that stretch, bounce, and deform realistically
**Unlike** the current state where particles only interact through gravity and collision — they can't form structures or hold shapes
**Our solution** adds GPU-accelerated distance constraints and pressure-based soft bodies, enabling cloth draping under gravity, ropes swinging from anchor points, and squishy blobs that deform when poked and bounce back into shape

---

Physicall now supports connected particle structures. Users can switch between simulation presets to explore different physical behaviors: a cloth sheet that drapes and folds under gravity, a rope that swings from a fixed point, or a soft body blob that deforms when dragged and restores its shape through internal pressure. Each structure is built from the same particle system — particles linked by spring constraints that try to maintain fixed distances. The constraints stretch and compress as forces act on the particles, creating realistic deformation. Soft bodies add internal pressure that pushes outward, giving structures volume and elasticity. Users interact with these structures using the same controls they already know — drag to pull, spawn to add, pause to freeze — and watch the physics play out in real time on the GPU.

## FAQ

### Customer Experience

Q: How does a user access these new simulation types?
A: A new simulation preset selector lets users switch between the existing N-body simulation and the new constraint-based presets (cloth, rope, soft body). Each preset initializes particles in the appropriate arrangement with constraints already connected.

Q: What does cloth simulation look like?
A: A grid of particles connected by distance constraints, pinned at the top corners. The cloth drapes downward under gravity, swings when dragged, and folds on itself when it contacts the floor. Particles collide with each other so the cloth doesn't pass through itself.

Q: What does a soft body look like?
A: A ring of particles forming a closed shape (like a 2D circle outline), connected by distance constraints along the perimeter and cross-bracing constraints for structure. Internal pressure pushes outward, giving it volume. When dragged or dropped, it squishes on impact and bounces back toward its resting shape.

Q: Can users interact with constrained structures?
A: Yes. Drag any particle in a cloth or soft body and the connected particles follow through the constraints. The existing mouse interactions (drag, spawn, delete) work on constrained particles just like free particles. Deleting a particle breaks its constraints, tearing the structure.

Q: Can users adjust constraint parameters?
A: Yes. New sliders control constraint stiffness (how rigid the connections are) and, for soft bodies, internal pressure. Low stiffness makes structures stretchy and elastic; high stiffness makes them rigid. These parameters update in real time.

Q: Does the existing N-body simulation still work?
A: Yes. The preset selector defaults to the N-body simulation. All existing functionality (gravity, collisions, mouse interaction, UI controls) is unchanged. Constraints are an addition, not a replacement.

### Scope & Non-Goals

Q: What is explicitly NOT included in V1?
A: No 3D cloth (only 2D grid draped in 3D space). No tearing based on stress thresholds — particles can only be deleted manually. No self-collision for cloth (particle-particle collision handles some of this but isn't cloth-specific). No 3D soft bodies (only 2D cross-section shapes). No joint constraints (hinges, sliders). No user-created custom constraint topologies — only presets.

Q: What's the minimal version that still delivers the core benefit?
A: Distance constraints between particles with a single preset (cloth grid). Even without soft bodies or pressure, a draggable cloth sheet demonstrates the core concept of connected particle structures.

Q: What would make us cut scope further if needed?
A: Drop soft bodies entirely and ship only distance constraints with a cloth preset. Soft bodies require pressure computation which is an additional layer of complexity. Cloth alone is a significant and visible addition.

Q: What are we deferring to future iterations?
A: 3D volumetric soft bodies, stress-based tearing, user-drawn constraint topologies, joint/hinge constraints, breakable constraints, and rope creation by click-and-drag.

### Technical Constraint FAQs

Q: What existing system capabilities does this depend on?
A: The GPU compute pipeline (E2.1), the particle data structure and buffers (E2.2), collision detection (E4.1), and the UI slider infrastructure (E3.1). Constraints are an additional compute pass that runs after gravity/collision.

Q: What new capabilities need to be built?
A: (1) A constraint data structure stored in GPU buffers — pairs of particle indices with rest lengths. (2) A constraint solver compute shader that iterates over constraints and adjusts particle positions to satisfy distance targets. (3) A pressure computation for soft bodies that calculates enclosed area and applies outward force. (4) Preset initialization code that creates particle arrangements with appropriate constraints. (5) A simulation mode selector in the UI.

Q: How does constraint solving work on the GPU?
A: Each constraint is a pair of particle indices and a rest length. The solver reads both particles' positions, computes the current distance, and moves each particle halfway toward satisfying the rest length. This is a Jacobi-style parallel solver — all constraints are solved simultaneously from the same input positions, then results are written out. Multiple iterations improve accuracy.

Q: How many constraint solver iterations per frame?
A: 4-8 iterations is typical for visually plausible results. More iterations = stiffer behavior but higher GPU cost. This should be configurable via a slider or constant.

Q: How does pressure work for soft bodies?
A: Calculate the enclosed area of the particle ring (using the shoelace formula on particle positions). Compare to a target area (rest area). If the current area is smaller than the target, apply outward normal forces to each edge particle proportional to the pressure deficit. This gives the "bouncy blob" behavior.

Q: Will constraints work with the existing collision system?
A: Yes. Collision runs first (detecting overlap and applying impulses), then constraint solving runs (adjusting positions to satisfy distances). The order matters — constraints get the "last word" on positions, which prevents constrained particles from being pushed apart by collision in ways that break the structure.

### Edge Cases & Error States

Q: What happens when a constrained particle is deleted?
A: The constraints referencing that particle become invalid and are skipped. For cloth, this creates a hole. For soft bodies, this breaks the perimeter — the shape loses pressure containment and collapses.

Q: What happens when a constrained particle is dragged?
A: The dragged particle moves toward the mouse target. Its constraints pull neighboring particles along, creating a ripple effect through the structure. This is the primary way users interact with cloth and soft bodies.

Q: What happens if constraints are over-stretched?
A: With a Jacobi solver and limited iterations, very stretched constraints won't fully restore in one frame — they'll gradually pull back over multiple frames, appearing elastic. This is acceptable and actually looks natural for cloth and soft bodies.

Q: What happens if many particles pile up on a constrained structure?
A: The collision system handles particle-structure interaction. Free particles landing on cloth will push it down; the constraints resist and the cloth deforms. With limited solver iterations, extreme loads may cause the cloth to stretch visibly but it won't break (no stress-based tearing in V1).

Q: What happens at simulation reset?
A: Reset restores the current preset's initial particle positions and constraint connections. Deleted particles and broken constraints are restored.

### Business & Operational FAQs

Q: How will we know if this is successful?
A: Users can see cloth draping under gravity and responding to drag interaction. Soft bodies visibly deform on impact and restore their shape. Constraint-based simulations run at >30 FPS with their default particle counts. The system feels physically plausible — structures hold together, stretch naturally, and respond to forces.

Q: What could go wrong post-launch?
A: Constraint solver instability causing structures to explode (too few iterations or too high stiffness). Performance regression from multiple solver iterations per frame. Interactions between collision and constraints causing jitter. Soft body pressure calculation producing unnatural "breathing" oscillations.

Q: What's the key learning outcome?
A: Understanding constraint-based physics simulation on the GPU — Position-Based Dynamics (PBD), iterative constraint solving, and how pressure models create emergent soft body behavior from simple distance constraints.

## Implementation Scope

### Current State -> End State

| Current State | End State |
| --- | --- |
| Particles are free-floating — they collide and attract but can't form structures | Particles can be linked by distance constraints to form cloth, ropes, and soft bodies |
| Only one simulation type (N-body gravity) | Multiple presets: N-body, cloth, rope, soft body |
| No parameter controls for structure behavior | Sliders for constraint stiffness and pressure |
| Particle deletion has no structural effect | Deleting a constrained particle tears the structure |
| Reset restores the same N-body ring distribution | Reset restores the active preset's initial configuration |

### User Stories

- [ ] As a user, I can select a simulation preset (N-body, cloth, rope, soft body) so that I can explore different physics behaviors
- [ ] As a user, I see a cloth sheet draping under gravity so that I can observe how connected particles behave as a fabric
- [ ] As a user, I can drag a cloth particle and watch the fabric deform and ripple so that constraint interactions feel tangible
- [ ] As a user, I see a soft body that deforms on impact and restores its shape so that I can observe pressure-based physics
- [ ] As a user, I can adjust constraint stiffness with a slider so that I can explore the range from stretchy to rigid
- [ ] As a user, I can adjust soft body pressure so that I can see how internal forces affect shape
- [ ] As a user, I can delete a particle from a structure and see it tear so that the simulation feels interactive and consequential
- [ ] As a user, all existing controls (pause, reset, camera, stats) work with the new simulation types so that the experience is consistent

### Success Criteria

- [ ] Cloth preset: grid of particles connected by distance constraints drapes under gravity when simulation starts
- [ ] Cloth responds to drag interaction — pulling a particle deforms the sheet and neighboring particles follow
- [ ] Soft body preset: closed particle ring with pressure maintains its shape and deforms elastically on impact
- [ ] Constraint solver runs on GPU with 4+ iterations per frame
- [ ] Cloth preset (30x30 grid, ~900 particles) runs at >30 FPS
- [ ] Preset selector switches between N-body and constraint-based simulations
- [ ] Stiffness and pressure sliders adjust behavior in real time
- [ ] Existing N-body simulation is unaffected — same behavior, same defaults

### Known Compromises

- [ ] Jacobi solver (parallel but less accurate) rather than Gauss-Seidel (sequential but more accurate) — required for GPU parallelism, visually sufficient with enough iterations
- [ ] 2D constraint topologies only (grids, rings) rather than full 3D volumetric structures — keeps mesh generation and pressure computation simple
- [ ] No stress-based tearing — constraints stretch infinitely rather than breaking under load. Manual deletion is the only way to tear structures
- [ ] Fixed presets only — users can't draw or define custom constraint layouts
- [ ] Soft body pressure uses 2D area calculation (shoelace formula) rather than 3D volume — matches the 2D cross-section soft body shape

### Open Questions / Needs Research

- [ ] What GPU buffer layout works best for constraints — array of (index_a, index_b, rest_length) structs?
- [ ] How to handle constraint solver iterations efficiently — multiple dispatch calls per frame, or loop within the shader?
- [ ] Should constraints be stored as a separate buffer or integrated into the particle buffer?
- [ ] How to structure the preset system — separate initialization functions per preset, or a configuration-driven approach?
- [ ] What default stiffness and pressure values feel good for each preset?
- [ ] How does the constraint solver interact with the existing double-buffered particle system — does it need its own ping-pong buffers?

---

## Next Steps

When ready to implement:

1. Run `/research E4 constraints-soft-bodies` to analyze codebase
2. Run `/plan E4 constraints-soft-bodies` to create implementation plan
3. Run `/implement [plan-path]` to execute the plan
