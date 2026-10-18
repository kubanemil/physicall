# physicall

Cloth, a rope, a box of grains and N-body gravity, simulated on the GPU in the browser. The
physics is Rust compiled to WebAssembly and runs as WebGPU compute shaders through wgpu. Cloth,
rope and grains use XPBD with small substeps; gravity is direct summation with leapfrog.

It runs live at https://kubanemil.github.io/physicall/ in any browser with WebGPU.

![Cloth pinned at two corners, grains settled in a box, a swinging rope, an N-body cluster](media/scenes.png)

A shader can be wrong and still look right, so every kernel also exists on the CPU.
`physicall-core` has no GPU code and runs the same passes in the same order. Its tests compare it
with results worked out on paper: a pendulum's period to 0.1%, a hanging chain's catenary to
1.5 mm, a compliant link's stretch to Hooke's law within 1%, and a Kepler orbit's energy error
under 2×10⁻⁴ over 20 orbits.

`physicall-gpu` runs the same scenes on a GPU and checks them against the CPU: the rope stays
within 10⁻⁶ m after 60 steps, 1000 bodies within 10⁻⁶ after 30, and the spatial hash matches
bucket for bucket. CI runs these on lavapipe, Mesa's software Vulkan driver.

To build it you need Rust with the `wasm32-unknown-unknown` target, wasm-pack and Node 24.

```
./do dev     # build the wasm package and serve the page on localhost:5173
./do test    # every test; GPU tests skip without an adapter
./do check   # rustfmt, clippy for native and wasm32, tsc
```

Set `PHYSICALL_REQUIRE_GPU=1` to make a missing adapter fail the GPU tests, as CI does.

MIT licensed.
