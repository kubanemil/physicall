// One colour of distance constraints. Constraints within a colour share no
// moving particle, so this is a race-free Gauss-Seidel sweep.

@group(1) @binding(0) var<uniform> colour: ColourRange;
@group(1) @binding(1) var<storage, read> constraints: array<GpuConstraint>;

@compute @workgroup_size(WORKGROUP_SIZE)
fn solve_distance(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= colour.count {
        return;
    }
    let c = constraints[colour.first + id.x];
    let pa = pos[c.a];
    let pb = pos[c.b];
    let w = pa.w + pb.w;
    let d = pa.xyz - pb.xyz;
    let len = length(d);
    if w == 0.0 || len == 0.0 || (c.material == MATERIAL_TETHER && len <= c.rest) {
        return;
    }
    let dlambda = -(len - c.rest) / (w + params.alpha[c.material]);
    let n = d / len;
    if pa.w > 0.0 {
        pos[c.a] = vec4<f32>(pa.xyz + n * (pa.w * dlambda), pa.w);
    }
    if pb.w > 0.0 {
        pos[c.b] = vec4<f32>(pb.xyz + n * (-pb.w * dlambda), pb.w);
    }
}
