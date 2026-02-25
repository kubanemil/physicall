/// Manages the depth texture for proper 3D rendering with depth testing.
pub(crate) struct DepthTexture {
    pub(crate) view: wgpu::TextureView,
    #[allow(dead_code)]
    texture: wgpu::Texture,
}

impl DepthTexture {
    /// Depth buffer format - 32-bit float for precision.
    pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// Creates a new depth texture with the given dimensions.
    pub(crate) fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }
}

/// Vertex with position and color for triangle rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position @ location(0)
        1 => Float32x3,  // color @ location(1)
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Line vertices for coordinate axes: X (red), Y (green), Z (blue).
/// Extended from -2 to +2 on each axis for better visibility.
pub(crate) const LINE_VERTICES: &[Vertex] = &[
    // X axis (red) - from -2 to +2
    Vertex { position: [-2.0, 0.0, 0.0], color: [0.5, 0.0, 0.0] },  // Darker for negative
    Vertex { position: [ 0.0, 0.0, 0.0], color: [0.5, 0.0, 0.0] },
    Vertex { position: [ 0.0, 0.0, 0.0], color: [1.0, 0.0, 0.0] },  // Brighter for positive
    Vertex { position: [ 2.0, 0.0, 0.0], color: [1.0, 0.0, 0.0] },
    // Y axis (green) - from -2 to +2
    Vertex { position: [0.0, -2.0, 0.0], color: [0.0, 0.5, 0.0] },
    Vertex { position: [0.0,  0.0, 0.0], color: [0.0, 0.5, 0.0] },
    Vertex { position: [0.0,  0.0, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.0,  2.0, 0.0], color: [0.0, 1.0, 0.0] },
    // Z axis (blue) - from -2 to +2
    Vertex { position: [0.0, 0.0, -2.0], color: [0.0, 0.0, 0.5] },
    Vertex { position: [0.0, 0.0,  0.0], color: [0.0, 0.0, 0.5] },
    Vertex { position: [0.0, 0.0,  0.0], color: [0.0, 0.0, 1.0] },
    Vertex { position: [0.0, 0.0,  2.0], color: [0.0, 0.0, 1.0] },
];

/// Quad vertex for point rendering - just 2D position for quad corners.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct QuadVertex {
    position: [f32; 2],
}

impl QuadVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
        0 => Float32x2,  // quad position @ location(0)
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Quad vertices: 4 corners of a unit quad (-1 to 1).
pub(crate) const QUAD_VERTICES: &[QuadVertex] = &[
    QuadVertex { position: [-1.0, -1.0] },
    QuadVertex { position: [ 1.0, -1.0] },
    QuadVertex { position: [-1.0,  1.0] },
    QuadVertex { position: [ 1.0,  1.0] },
];

/// Quad indices: 2 triangles forming the quad.
pub(crate) const QUAD_INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];
