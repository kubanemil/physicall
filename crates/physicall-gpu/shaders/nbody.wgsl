// Kick-drift-kick leapfrog with tiled direct-summation gravity (Nyland,
// Harris, Prins 2007, GPU Gems 3 ch. 31). pos.w is the inverse mass; acc.w
// holds the particle's potential for energy diagnostics.

@group(0) @binding(0) var<uniform> params: NBodyUniforms;
@group(0) @binding(1) var<storage, read_write> pos: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> vel: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> acc: array<vec4<f32>>;

@compute @workgroup_size(WORKGROUP_SIZE)
fn kick_drift(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let v = vel[i].xyz + acc[i].xyz * (0.5 * params.h);
    vel[i] = vec4<f32>(v, 0.0);
    let x = pos[i];
    pos[i] = vec4<f32>(x.xyz + v * params.h, x.w);
}

var<workgroup> tile: array<vec4<f32>, NBODY_TILE>;

@compute @workgroup_size(NBODY_TILE)
fn force_kick(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let i = id.x;
    let in_range = i < params.count;
    var xi = vec3<f32>(0.0);
    if in_range {
        xi = pos[i].xyz;
    }
    var a = vec3<f32>(0.0);
    var phi = 0.0;
    let tiles = (params.count + NBODY_TILE - 1u) / NBODY_TILE;
    for (var t = 0u; t < tiles; t++) {
        let first = t * NBODY_TILE;
        let j = first + lane;
        var body = vec4<f32>(0.0);
        if j < params.count {
            let q = pos[j];
            body = vec4<f32>(q.xyz, 1.0 / q.w);
        }
        tile[lane] = body;
        workgroupBarrier();
        let n = min(NBODY_TILE, params.count - first);
        for (var k = 0u; k < n; k++) {
            if first + k == i {
                continue;
            }
            let other = tile[k];
            let d = other.xyz - xi;
            let inv_r = 1.0 / sqrt(dot(d, d) + params.eps2);
            a += d * (other.w * inv_r * inv_r * inv_r);
            phi -= other.w * inv_r;
        }
        workgroupBarrier();
    }
    if in_range {
        a *= params.g;
        acc[i] = vec4<f32>(a, phi * params.g);
        vel[i] = vec4<f32>(vel[i].xyz + a * (0.5 * params.h), 0.0);
    }
}
