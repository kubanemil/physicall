// Draws straight from the simulation's storage buffers; nothing is copied
// back to the CPU to render a frame.

@group(0) @binding(0) var<uniform> view: RenderUniforms;
@group(0) @binding(1) var<storage, read> pos: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> vel: array<vec4<f32>>;

const STYLE_ALL: u32 = 0u;
const STYLE_STATIC_ONLY: u32 = 1u;

fn shade(base: vec3<f32>, normal: vec3<f32>, to_eye: vec3<f32>) -> vec3<f32> {
    let light = normalize(vec3<f32>(0.4, 0.85, 0.35));
    let diffuse = max(dot(normal, light), 0.0);
    let specular = pow(max(dot(normal, normalize(light + to_eye)), 0.0), 48.0) * 0.3;
    return base * (0.2 + 0.8 * diffuse) + vec3<f32>(specular);
}

fn speed_colour(speed: f32) -> vec3<f32> {
    let t = clamp(speed * view.speed_scale, 0.0, 1.0);
    let slow = vec3<f32>(0.18, 0.42, 0.92);
    let middle = vec3<f32>(0.32, 0.82, 0.74);
    let fast = vec3<f32>(1.0, 0.56, 0.2);
    if t < 0.5 {
        return mix(slow, middle, 2.0 * t);
    }
    return mix(middle, fast, 2.0 * t - 1.0);
}

struct SphereOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) colour: vec3<f32>,
}

@vertex
fn sphere_vs(@builtin(vertex_index) corner: u32, @builtin(instance_index) i: u32) -> SphereOut {
    var out: SphereOut;
    let p = pos[i];
    let fixed = p.w == 0.0;
    if view.style == STYLE_STATIC_ONLY && !fixed && i != view.highlight {
        out.clip = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        return out;
    }
    let uv = vec2<f32>(f32(corner & 1u), f32(corner >> 1u)) * 2.0 - 1.0;
    let world = p.xyz + (view.right.xyz * uv.x + view.up.xyz * uv.y) * view.radius;
    var colour = speed_colour(length(vel[i].xyz));
    if fixed {
        colour = vec3<f32>(0.93, 0.33, 0.36);
    }
    if i == view.highlight {
        colour = mix(colour, vec3<f32>(1.0), 0.5);
    }
    out.clip = view.view_proj * vec4<f32>(world, 1.0);
    out.uv = uv;
    out.colour = colour;
    return out;
}

@fragment
fn sphere_fs(in: SphereOut) -> @location(0) vec4<f32> {
    let r2 = dot(in.uv, in.uv);
    if r2 > 1.0 {
        discard;
    }
    let forward = cross(view.right.xyz, view.up.xyz);
    let normal = view.right.xyz * in.uv.x + view.up.xyz * in.uv.y + forward * sqrt(1.0 - r2);
    return vec4<f32>(shade(in.colour, normal, forward), 1.0);
}

@vertex
fn line_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return view.view_proj * vec4<f32>(pos[i].xyz, 1.0);
}

@fragment
fn line_fs() -> @location(0) vec4<f32> {
    return vec4<f32>(0.86, 0.8, 0.66, 1.0);
}

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn mesh_vs(@builtin(vertex_index) i: u32) -> MeshOut {
    var out: MeshOut;
    let p = pos[i].xyz;
    let n = max(view.columns, 2u);
    out.clip = view.view_proj * vec4<f32>(p, 1.0);
    out.world = p;
    out.uv = vec2<f32>(f32(i % n), f32(i / n)) / f32(n - 1u);
    return out;
}

@fragment
fn mesh_fs(in: MeshOut) -> @location(0) vec4<f32> {
    var normal = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let to_eye = normalize(view.eye.xyz - in.world);
    if dot(normal, to_eye) < 0.0 {
        normal = -normal;
    }
    let cell = vec2<u32>(floor(in.uv * 10.0));
    let light_square = (cell.x + cell.y) % 2u == 0u;
    let base = select(vec3<f32>(0.8, 0.3, 0.27), vec3<f32>(0.94, 0.88, 0.78), light_square);
    return vec4<f32>(shade(base, normal, to_eye), 1.0);
}

struct GridOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn grid_vs(@builtin(vertex_index) corner: u32) -> GridOut {
    var out: GridOut;
    let xz = (vec2<f32>(f32(corner & 1u), f32(corner >> 1u)) * 2.0 - 1.0) * 8.0;
    out.world = vec3<f32>(xz.x, view.floor, xz.y);
    out.clip = view.view_proj * vec4<f32>(out.world, 1.0);
    return out;
}

@fragment
fn grid_fs(in: GridOut) -> @location(0) vec4<f32> {
    let coord = in.world.xz * 4.0;
    let distance_to_line = abs(fract(coord - 0.5) - 0.5) / fwidth(coord);
    let line = 1.0 - min(min(distance_to_line.x, distance_to_line.y), 1.0);
    let fade = 1.0 - smoothstep(1.5, 7.0, length(in.world.xz));
    let base = vec3<f32>(0.085, 0.09, 0.105);
    return vec4<f32>(mix(base, vec3<f32>(0.24, 0.26, 0.3), line * fade), 1.0);
}

const BOX_EDGES = array<u32, 24>(0u, 1u, 1u, 3u, 3u, 2u, 2u, 0u, 4u, 5u, 5u, 7u, 7u, 6u, 6u, 4u, 0u, 4u, 1u, 5u, 2u, 6u, 3u, 7u);

@vertex
fn box_vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    let c = BOX_EDGES[v];
    let w = view.half_width;
    let corner = vec3<f32>(
        select(-w, w, (c & 1u) != 0u),
        select(view.floor, view.floor + 0.5, (c & 4u) != 0u),
        select(-w, w, (c & 2u) != 0u),
    );
    return view.view_proj * vec4<f32>(corner, 1.0);
}

@fragment
fn box_fs() -> @location(0) vec4<f32> {
    return vec4<f32>(0.36, 0.4, 0.48, 1.0);
}
