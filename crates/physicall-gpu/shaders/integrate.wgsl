@compute @workgroup_size(WORKGROUP_SIZE)
fn predict(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let x = pos[i];
    prev[i] = x;
    if x.w == 0.0 {
        return;
    }
    let v = vel[i].xyz * params.damping + params.gravity.xyz * params.h;
    vel[i] = vec4<f32>(v, 0.0);
    pos[i] = vec4<f32>(x.xyz + v * params.h, x.w);
}

fn collide_plane(x: vec3<f32>, start: vec3<f32>, normal: vec3<f32>, offset: f32) -> vec3<f32> {
    let penetration = offset - dot(x, normal);
    if penetration <= 0.0 {
        return x;
    }
    let pushed = x + normal * penetration;
    let moved = pushed - start;
    let tangential = moved - normal * dot(moved, normal);
    return pushed - friction(tangential, penetration, params.friction);
}

// Collides with the container, then derives velocity from the substep's
// displacement, as XPBD does.
@compute @workgroup_size(WORKGROUP_SIZE)
fn finish(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    let p = pos[i];
    if p.w == 0.0 {
        vel[i] = vec4<f32>(0.0);
        return;
    }
    let start = prev[i].xyz;
    var x = p.xyz;
    if (params.flags & FLAG_FLOOR) != 0u {
        x = collide_plane(x, start, vec3<f32>(0.0, 1.0, 0.0), params.floor + params.radius);
    }
    if (params.flags & FLAG_WALLS) != 0u {
        let offset = params.radius - params.half_width;
        x = collide_plane(x, start, vec3<f32>(1.0, 0.0, 0.0), offset);
        x = collide_plane(x, start, vec3<f32>(-1.0, 0.0, 0.0), offset);
        x = collide_plane(x, start, vec3<f32>(0.0, 0.0, 1.0), offset);
        x = collide_plane(x, start, vec3<f32>(0.0, 0.0, -1.0), offset);
    }
    pos[i] = vec4<f32>(x, p.w);
    vel[i] = vec4<f32>((x - start) * params.inv_h, 0.0);
}
