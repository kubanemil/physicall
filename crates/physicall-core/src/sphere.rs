/// Vertex for sphere mesh with position and normal.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SphereVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl SphereVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position @ location(0)
        1 => Float32x3,  // normal @ location(1)
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SphereVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Per-instance data for sphere rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SphereInstance {
    position: [f32; 3],
    radius: f32,
    color: [f32; 3],
    _padding: f32,
}

impl SphereInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        2 => Float32x3,  // world_pos @ location(2)
        3 => Float32,    // radius @ location(3)
        4 => Float32x3,  // color @ location(4)
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SphereInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Generates an icosphere mesh with the given number of subdivisions.
/// Returns (vertices, indices).
/// Level 0 = 12 vertices, 20 triangles (base icosahedron)
/// Level 1 = 42 vertices, 80 triangles
/// Level 2 = 162 vertices, 320 triangles
pub(crate) fn generate_icosphere(subdivisions: u32) -> (Vec<SphereVertex>, Vec<u32>) {
    use std::collections::BTreeMap;

    // Golden ratio for icosahedron construction
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;

    // Normalize to unit sphere
    let len = (1.0 + t * t).sqrt();
    let a = 1.0 / len;
    let b = t / len;

    // 12 vertices of icosahedron (3 orthogonal golden rectangles)
    let mut vertices: Vec<[f32; 3]> = vec![
        [-a,  b,  0.0], [ a,  b,  0.0], [-a, -b,  0.0], [ a, -b,  0.0],
        [ 0.0, -a,  b], [ 0.0,  a,  b], [ 0.0, -a, -b], [ 0.0,  a, -b],
        [ b,  0.0, -a], [ b,  0.0,  a], [-b,  0.0, -a], [-b,  0.0,  a],
    ];

    // 20 triangles of icosahedron
    let mut indices: Vec<[u32; 3]> = vec![
        // 5 faces around vertex 0
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        // 5 adjacent faces
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        // 5 faces around vertex 3
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        // 5 adjacent faces
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    // Subdivide
    for _ in 0..subdivisions {
        let mut new_indices: Vec<[u32; 3]> = Vec::new();
        let mut edge_cache: BTreeMap<(u32, u32), u32> = BTreeMap::new();

        for tri in &indices {
            let v0 = tri[0];
            let v1 = tri[1];
            let v2 = tri[2];

            // Get or create midpoint vertices
            let m01 = get_midpoint(v0, v1, &mut vertices, &mut edge_cache);
            let m12 = get_midpoint(v1, v2, &mut vertices, &mut edge_cache);
            let m20 = get_midpoint(v2, v0, &mut vertices, &mut edge_cache);

            // Create 4 new triangles
            new_indices.push([v0, m01, m20]);
            new_indices.push([v1, m12, m01]);
            new_indices.push([v2, m20, m12]);
            new_indices.push([m01, m12, m20]);
        }

        indices = new_indices;
    }

    // Convert to SphereVertex (normal = position for unit sphere)
    let sphere_vertices: Vec<SphereVertex> = vertices
        .iter()
        .map(|&pos| SphereVertex {
            position: pos,
            normal: pos, // For unit sphere, normal equals position
        })
        .collect();

    // Flatten indices
    let flat_indices: Vec<u32> = indices.iter().flat_map(|tri| tri.iter().copied()).collect();

    (sphere_vertices, flat_indices)
}

/// Gets or creates a midpoint vertex between two vertices, normalized to unit sphere.
fn get_midpoint(
    v0: u32,
    v1: u32,
    vertices: &mut Vec<[f32; 3]>,
    cache: &mut std::collections::BTreeMap<(u32, u32), u32>,
) -> u32 {
    // Use sorted key to ensure consistent lookup regardless of edge direction
    let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };

    if let Some(&idx) = cache.get(&key) {
        return idx;
    }

    // Calculate midpoint
    let p0 = vertices[v0 as usize];
    let p1 = vertices[v1 as usize];
    let mid = [
        (p0[0] + p1[0]) / 2.0,
        (p0[1] + p1[1]) / 2.0,
        (p0[2] + p1[2]) / 2.0,
    ];

    // Normalize to unit sphere
    let len = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
    let normalized = [mid[0] / len, mid[1] / len, mid[2] / len];

    // Add new vertex
    let new_idx = vertices.len() as u32;
    vertices.push(normalized);
    cache.insert(key, new_idx);

    new_idx
}

/// Creates demo sphere instances for testing.
pub(crate) fn create_demo_spheres() -> Vec<SphereInstance> {
    vec![
        // Central large sphere (white)
        SphereInstance {
            position: [0.0, 0.0, -0.5],
            radius: 0.3,
            color: [0.9, 0.9, 0.9],
            _padding: 0.0,
        },
        // Red sphere
        SphereInstance {
            position: [0.8, 0.2, 0.0],
            radius: 0.2,
            color: [1.0, 0.3, 0.3],
            _padding: 0.0,
        },
        // Green sphere
        SphereInstance {
            position: [-0.7, -0.3, 0.2],
            radius: 0.25,
            color: [0.3, 1.0, 0.3],
            _padding: 0.0,
        },
        // Blue sphere
        SphereInstance {
            position: [0.3, 0.8, 0.3],
            radius: 0.15,
            color: [0.3, 0.3, 1.0],
            _padding: 0.0,
        },
        // Yellow sphere
        SphereInstance {
            position: [-0.5, 0.6, -0.3],
            radius: 0.18,
            color: [1.0, 1.0, 0.3],
            _padding: 0.0,
        },
        // Cyan sphere
        SphereInstance {
            position: [0.6, -0.6, 0.4],
            radius: 0.22,
            color: [0.3, 1.0, 1.0],
            _padding: 0.0,
        },
        // Magenta sphere
        SphereInstance {
            position: [-0.8, 0.0, 0.5],
            radius: 0.12,
            color: [1.0, 0.3, 1.0],
            _padding: 0.0,
        },
        // Orange sphere
        SphereInstance {
            position: [0.0, -0.8, -0.2],
            radius: 0.2,
            color: [1.0, 0.6, 0.2],
            _padding: 0.0,
        },
    ]
}
