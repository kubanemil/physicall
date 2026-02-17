// Hash clear shader - zeros out spatial hash cell counters
// Dispatched as ceil(NUM_CELLS / 256) workgroups

@group(1) @binding(0) var<storage, read_write> cell_count: array<atomic<u32>>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if (idx >= arrayLength(&cell_count)) {
        return;
    }
    atomicStore(&cell_count[idx], 0u);
}
