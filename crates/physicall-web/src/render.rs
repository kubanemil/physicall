use wgpu::util::DeviceExt;

physicall_gpu::gpu_structs! {
    const RENDER_STRUCTS;

    pub struct RenderUniforms {
        pub view_proj: [[f32; 4]; 4],
        pub right: [f32; 4],
        pub up: [f32; 4],
        pub eye: [f32; 4],
        pub radius: f32,
        pub highlight: u32,
        pub style: u32,
        pub speed_scale: f32,
        pub floor: f32,
        pub half_width: f32,
        pub columns: u32,
        pub pad: u32,
    }
}

pub const STYLE_ALL: u32 = 0;
pub const STYLE_STATIC_ONLY: u32 = 1;

pub const RENDER_SHADER: &str =
    physicall_gpu::concat_str!(RENDER_STRUCTS, include_str!("../shaders/render.wgsl"));

const SAMPLES: u32 = 4;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.038,
    b: 0.048,
    a: 1.0,
};

/// What one scene draws, bound to its simulation buffers.
pub struct Drawable {
    pub bind_group: wgpu::BindGroup,
    pub count: u32,
    pub spheres: bool,
    pub floor: bool,
    pub walls: bool,
    pub lines: Option<(wgpu::Buffer, u32)>,
    pub triangles: Option<(wgpu::Buffer, u32)>,
}

pub struct Renderer {
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    sphere: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    mesh: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    walls: wgpu::RenderPipeline,
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1),
                storage(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render"),
            source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()),
        });
        let pipeline = |name: &str, topology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(&format!("{name}_vs")),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(&format!("{name}_fs")),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let (msaa, depth) = targets(device, format, width, height);
        Self {
            uniforms: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render uniforms"),
                contents: bytemuck::bytes_of(&RenderUniforms::default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }),
            sphere: pipeline("sphere", wgpu::PrimitiveTopology::TriangleStrip),
            line: pipeline("line", wgpu::PrimitiveTopology::LineList),
            mesh: pipeline("mesh", wgpu::PrimitiveTopology::TriangleList),
            grid: pipeline("grid", wgpu::PrimitiveTopology::TriangleStrip),
            walls: pipeline("box", wgpu::PrimitiveTopology::LineList),
            layout,
            msaa,
            depth,
        }
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        (self.msaa, self.depth) = targets(device, format, width, height);
    }

    pub fn bind(
        &self,
        device: &wgpu::Device,
        pos: &wgpu::Buffer,
        vel: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: pos.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vel.as_entire_binding(),
                },
            ],
        })
    }

    pub fn index_buffer(device: &wgpu::Device, indices: &[u32]) -> Option<(wgpu::Buffer, u32)> {
        (!indices.is_empty()).then(|| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("indices"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            (buffer, indices.len() as u32)
        })
    }

    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        drawable: &Drawable,
        uniforms: &RenderUniforms,
    ) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.msaa,
                depth_slice: None,
                resolve_target: Some(target),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(BACKGROUND),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &drawable.bind_group, &[]);
        if drawable.floor {
            pass.set_pipeline(&self.grid);
            pass.draw(0..4, 0..1);
        }
        if drawable.walls {
            pass.set_pipeline(&self.walls);
            pass.draw(0..24, 0..1);
        }
        if let Some((indices, count)) = &drawable.triangles {
            pass.set_pipeline(&self.mesh);
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*count, 0, 0..1);
        }
        if let Some((indices, count)) = &drawable.lines {
            pass.set_pipeline(&self.line);
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*count, 0, 0..1);
        }
        if drawable.spheres {
            pass.set_pipeline(&self.sphere);
            pass.draw(0..4, 0..drawable.count);
        }
    }
}

fn targets(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::TextureView, wgpu::TextureView) {
    let texture = |label, format| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: SAMPLES,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    };
    (texture("msaa", format), texture("depth", DEPTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_shader_validates() {
        let module = naga::front::wgsl::parse_str(RENDER_SHADER)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(RENDER_SHADER)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(RENDER_SHADER)));
    }
}
