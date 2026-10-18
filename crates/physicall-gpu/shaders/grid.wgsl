// Counting-sort spatial hash (clear, count, scan, scatter) and the Jacobi
// particle contact pass that reads it.

@group(1) @binding(0) var<storage, read_write> cell_start: array<atomic<u32>>;
@group(1) @binding(1) var<storage, read_write> entries: array<vec2<u32>>;
@group(1) @binding(2) var<storage, read_write> sorted: array<u32>;
@group(1) @binding(3) var<storage, read_write> delta: array<vec4<f32>>;

fn cell_of(p: vec3<f32>) -> vec3<i32> {
    return vec3<i32>(floor(p / (2.0 * params.radius)));
}

// Teschner et al. 2003; u32 multiplication wraps like Rust's wrapping_mul.
fn bucket(cell: vec3<i32>) -> u32 {
    let c = bitcast<vec3<u32>>(cell);
    return ((c.x * HASH_PRIME_X) ^ (c.y * HASH_PRIME_Y) ^ (c.z * HASH_PRIME_Z)) % params.table_size;
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn hash_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x <= params.table_size {
        atomicStore(&cell_start[id.x], 0u);
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn hash_count(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let b = bucket(cell_of(pos[i].xyz));
    entries[i] = vec2<u32>(b, atomicAdd(&cell_start[b], 1u));
}

var<workgroup> partial: array<u32, SCAN_THREADS>;

// Exclusive prefix sum of bucket counts in one workgroup: each thread sums a
// contiguous run, a Hillis-Steele scan combines the runs, then each thread
// writes its run's offsets. cell_start[table_size] ends up as the total.
@compute @workgroup_size(SCAN_THREADS)
fn hash_scan(@builtin(local_invocation_index) t: u32) {
    let n = params.table_size;
    let per = (n + SCAN_THREADS - 1u) / SCAN_THREADS;
    let begin = min(t * per, n);
    let end = min(begin + per, n);
    var sum = 0u;
    for (var c = begin; c < end; c++) {
        sum += atomicLoad(&cell_start[c]);
    }
    partial[t] = sum;
    workgroupBarrier();
    for (var offset = 1u; offset < SCAN_THREADS; offset <<= 1u) {
        var add = 0u;
        if t >= offset {
            add = partial[t - offset];
        }
        workgroupBarrier();
        partial[t] += add;
        workgroupBarrier();
    }
    var running = partial[t] - sum;
    for (var c = begin; c < end; c++) {
        let count = atomicLoad(&cell_start[c]);
        atomicStore(&cell_start[c], running);
        running += count;
    }
    if t == SCAN_THREADS - 1u {
        atomicStore(&cell_start[n], partial[t]);
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn hash_scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let e = entries[i];
    sorted[atomicLoad(&cell_start[e.x]) + e.y] = i;
}

// Atomics fill each bucket in arbitrary order. Sorting by index makes contact
// sums add up in the CPU's order, so runs are reproducible. Buckets hold a
// handful of particles, so insertion sort is enough.
@compute @workgroup_size(WORKGROUP_SIZE)
fn hash_sort(@builtin(global_invocation_id) id: vec3<u32>) {
    let b = id.x;
    if b >= params.table_size {
        return;
    }
    let begin = atomicLoad(&cell_start[b]);
    let end = atomicLoad(&cell_start[b + 1u]);
    for (var s = begin + 1u; s < end; s++) {
        let value = sorted[s];
        var t = s;
        while t > begin && sorted[t - 1u] > value {
            sorted[t] = sorted[t - 1u];
            t--;
        }
        sorted[t] = value;
    }
}

// Jacobi contact correction with constraint averaging (Macklin et al. 2014,
// sec. 4.2). Reads positions only; apply_delta writes them back.
@compute @workgroup_size(WORKGROUP_SIZE)
fn contacts(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let p = pos[i];
    if p.w == 0.0 {
        delta[i] = vec4<f32>(0.0);
        return;
    }
    let xi = p.xyz;
    let moved_i = xi - prev[i].xyz;
    let contact = 2.0 * params.radius;
    let base = cell_of(xi);

    var visited: array<u32, 27>;
    var visited_count = 0u;
    var sum = vec3<f32>(0.0);
    var count = 0u;
    for (var dz = -1; dz <= 1; dz++) {
        for (var dy = -1; dy <= 1; dy++) {
            for (var dx = -1; dx <= 1; dx++) {
                let b = bucket(base + vec3<i32>(dx, dy, dz));
                var seen = false;
                for (var k = 0u; k < visited_count; k++) {
                    seen = seen || visited[k] == b;
                }
                if seen {
                    continue;
                }
                visited[visited_count] = b;
                visited_count++;

                let end = atomicLoad(&cell_start[b + 1u]);
                for (var s = atomicLoad(&cell_start[b]); s < end; s++) {
                    let j = sorted[s];
                    if j == i {
                        continue;
                    }
                    let q = pos[j];
                    let d = xi - q.xyz;
                    let dist2 = dot(d, d);
                    if dist2 >= contact * contact || dist2 == 0.0 {
                        continue;
                    }
                    let dist = sqrt(dist2);
                    let normal = d / dist;
                    let penetration = contact - dist;
                    let share = p.w / (p.w + q.w);
                    let relative = moved_i - (q.xyz - prev[j].xyz);
                    let tangential = relative - normal * dot(relative, normal);
                    sum += (normal * penetration - friction(tangential, penetration, params.friction)) * share;
                    count++;
                }
            }
        }
    }

    var out = vec3<f32>(0.0);
    if count > 0u {
        out = sum * (params.relaxation / f32(count));
    }
    delta[i] = vec4<f32>(out, 0.0);
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn apply_delta(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let p = pos[i];
    pos[i] = vec4<f32>(p.xyz + delta[i].xyz, p.w);
}
