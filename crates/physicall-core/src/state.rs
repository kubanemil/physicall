use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::camera::{CameraUniform, OrbitCamera, Projection};
use crate::particle::*;
use crate::ray::{ray_sphere_intersection, screen_to_world_ray};
use crate::render::*;
use crate::sphere::*;

/// Holds all wgpu resources needed for rendering.
pub(crate) struct State {
    surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub(crate) size: (u32, u32),
    // Camera system
    pub(crate) camera: OrbitCamera,
    pub(crate) projection: Projection,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // Depth buffer
    depth_texture: DepthTexture,
    // Line rendering
    line_pipeline: wgpu::RenderPipeline,
    line_vertex_buffer: wgpu::Buffer,
    line_vertex_count: u32,
    // Particle system (compute + render)
    pub(crate) particle_buffers: [wgpu::Buffer; 2],
    pub(crate) active_particle_count: u32,
    pub(crate) max_particles: u32,
    compute_pipeline: wgpu::ComputePipeline,
    hash_clear_pipeline: wgpu::ComputePipeline,
    hash_insert_pipeline: wgpu::ComputePipeline,
    compute_bind_groups: [wgpu::BindGroup; 2],
    hash_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    cell_count_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    cell_entries_buffer: wgpu::Buffer,
    sim_params_buffer: wgpu::Buffer,
    frame_index: u32,
    last_frame_time: Option<f64>,
    // Point rendering (instanced quads)
    point_pipeline: wgpu::RenderPipeline,
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    // Sphere rendering (instanced icosphere)
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_vertex_buffer: wgpu::Buffer,
    sphere_index_buffer: wgpu::Buffer,
    sphere_index_count: u32,
    sphere_instance_buffer: wgpu::Buffer,
    sphere_instance_count: u32,
    // UI Controls state
    pub(crate) paused: bool,
    pub(crate) gravity: f32,
    pub(crate) time_scale: f32,
    pub(crate) initial_speed: f32,
    initial_particles: Vec<Particle>,
    initial_particle_count: u32,
    // CPU-side particle positions for picking
    pub(crate) particle_positions: Vec<Vec3>,
    // Particle interaction state
    pub(crate) hovered_particle_idx: Option<u32>,
    pub(crate) dragged_particle_idx: Option<u32>,
    pub(crate) drag_target: Option<Vec3>,
    // Point rendering bind group (includes camera + sim_params)
    point_bind_group: wgpu::BindGroup,
}

impl State {
    /// Creates a new State with WebGPU initialized for the given canvas.
    pub(crate) async fn new(canvas: web_sys::HtmlCanvasElement) -> Self {
        let width = canvas.width();
        let height = canvas.height();

        // Create wgpu instance with WebGPU backend
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        // Create surface from canvas
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .expect("Failed to create surface from canvas");

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        // Request device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("Failed to create GPU device");

        // Get surface capabilities and configure
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Initialize camera system
        let camera = OrbitCamera::new();
        let projection = Projection::new(width, height);
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update(&camera, &projection);

        // Create camera uniform buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create camera bind group layout
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create camera bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout with camera bind group
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            ..Default::default()
        });

        // Create depth texture
        let depth_texture = DepthTexture::new(&device, width, height);

        // ====================================================================
        // Line Pipeline Setup
        // ====================================================================

        // Load line shader
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Line Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/line.wgsl").into()),
        });

        // Create line render pipeline (uses same layout as triangle pipeline)
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Line Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Lines have no faces to cull
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // Create line vertex buffer
        let line_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Vertex Buffer"),
            contents: bytemuck::cast_slice(LINE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let line_vertex_count = LINE_VERTICES.len() as u32;

        // ====================================================================
        // Simulation Parameters (needed by point pipeline)
        // ====================================================================

        // Create simulation parameters uniform buffer
        let sim_params = SimParams {
            delta_time: 1.0 / 60.0,
            gravitational_constant: GRAVITATIONAL_CONSTANT,
            damping: DAMPING,
            particle_count: PARTICLE_COUNT,
            softening_sq: SOFTENING_SQ,
            drag_strength: 0.0,
            dragged_particle_idx: -1,
            hovered_particle_idx: -1,
            drag_target: [0.0; 3],
            restitution: RESTITUTION,
            floor_y: FLOOR_Y,
            cell_size: CELL_SIZE,
            num_cells: NUM_CELLS,
            _padding: 0.0,
        };
        let sim_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Simulation Params Buffer"),
            contents: bytemuck::cast_slice(&[sim_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create point bind group layout (camera + sim_params for point shader)
        let point_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Point Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let point_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Point Bind Group"),
            layout: &point_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sim_params_buffer.as_entire_binding(),
                },
            ],
        });

        let point_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Point Pipeline Layout"),
                bind_group_layouts: &[&point_bind_group_layout],
                ..Default::default()
            });

        // ====================================================================
        // Point Pipeline Setup (Instanced Quads)
        // ====================================================================

        // Load point shader
        let point_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Point Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/point.wgsl").into()),
        });

        // Create point render pipeline with two vertex buffers (quad + particle instance)
        let point_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Point Pipeline"),
            layout: Some(&point_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &point_shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::desc(), Particle::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &point_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Don't cull - quads should be visible from both sides
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // Create quad vertex buffer (shared geometry for all points)
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create quad index buffer
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ====================================================================
        // Particle System Setup (Compute + Render Buffer Sharing)
        // ====================================================================

        // Create initial particles (clone for reset capability)
        let initial_particles = create_initial_particles(PARTICLE_COUNT, INITIAL_SPEED);
        let initial_particle_count = initial_particles.len() as u32;
        let active_particle_count = initial_particle_count;

        // Buffer usage for compute-render sharing (COPY_SRC needed for delete-by-swap)
        let particle_buffer_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        // Pre-allocate buffers for MAX_PARTICLES capacity
        let buffer_size = (MAX_PARTICLES as usize) * std::mem::size_of::<Particle>();

        // Create double-buffered particle storage (ping-pong pattern)
        // Initialize with zeros, then write initial particles
        let particle_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer 0"),
                size: buffer_size as u64,
                usage: particle_buffer_usage,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer 1"),
                size: buffer_size as u64,
                usage: particle_buffer_usage,
                mapped_at_creation: false,
            }),
        ];

        // Write initial particles to both buffers
        queue.write_buffer(&particle_buffers[0], 0, bytemuck::cast_slice(&initial_particles));
        queue.write_buffer(&particle_buffers[1], 0, bytemuck::cast_slice(&initial_particles));

        // Create compute bind group layout (group 0: particles + sim_params)
        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    // Binding 0: Input particles (read-only storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Binding 1: Output particles (read-write storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Binding 2: Simulation parameters (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create hash bind group layout (group 1: spatial hash data)
        let hash_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Hash Bind Group Layout"),
                entries: &[
                    // Binding 0: Cell counts (read_write for clear/insert, read for physics)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Binding 1: Cell entries (read_write for insert, read for physics)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create spatial hash GPU buffers
        let cell_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cell Count Buffer"),
            size: (NUM_CELLS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cell_entries_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cell Entries Buffer"),
            size: (NUM_CELLS * MAX_PER_CELL * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Create hash bind group
        let hash_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hash Bind Group"),
            layout: &hash_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cell_count_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cell_entries_buffer.as_entire_binding(),
                },
            ],
        });

        // Shared compute pipeline layout: [compute_group, hash_group]
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout, &hash_bind_group_layout],
                ..Default::default()
            });

        // Load shaders
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/particle_compute.wgsl").into(),
            ),
        });
        let hash_clear_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hash Clear Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/hash_clear.wgsl").into(),
            ),
        });
        let hash_insert_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hash Insert Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/hash_insert.wgsl").into(),
            ),
        });

        // Create compute pipelines (all share the same 2-group layout)
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Particle Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let hash_clear_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Hash Clear Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &hash_clear_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let hash_insert_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Hash Insert Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &hash_insert_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Create two bind groups for ping-pong pattern (group 0)
        // bind_group_0: read from buffer[0], write to buffer[1]
        // bind_group_1: read from buffer[1], write to buffer[0]
        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group 0"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group 1"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        // ====================================================================
        // Sphere Pipeline Setup (Instanced Icosphere)
        // ====================================================================

        // Generate icosphere mesh (level 2 = 162 vertices, 320 triangles)
        let (sphere_vertices, sphere_indices) = generate_icosphere(2);
        let sphere_index_count = sphere_indices.len() as u32;

        // Load sphere shader
        let sphere_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sphere Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/sphere.wgsl").into()),
        });

        // Create sphere render pipeline
        let sphere_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sphere Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sphere_shader,
                entry_point: Some("vs_main"),
                buffers: &[SphereVertex::desc(), SphereInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sphere_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back), // Cull back faces for solid spheres
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // Create sphere vertex buffer
        let sphere_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Vertex Buffer"),
            contents: bytemuck::cast_slice(&sphere_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create sphere index buffer
        let sphere_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Index Buffer"),
            contents: bytemuck::cast_slice(&sphere_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create sphere instance buffer with demo data
        let demo_spheres = create_demo_spheres();
        let sphere_instance_count = demo_spheres.len() as u32;

        let sphere_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sphere Instance Buffer"),
            contents: bytemuck::cast_slice(&demo_spheres),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
            camera,
            projection,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            line_pipeline,
            line_vertex_buffer,
            line_vertex_count,
            particle_buffers,
            active_particle_count,
            max_particles: MAX_PARTICLES,
            compute_pipeline,
            hash_clear_pipeline,
            hash_insert_pipeline,
            compute_bind_groups,
            hash_bind_group,
            cell_count_buffer,
            cell_entries_buffer,
            sim_params_buffer,
            frame_index: 0,
            last_frame_time: None,
            point_pipeline,
            quad_vertex_buffer,
            quad_index_buffer,
            sphere_pipeline,
            sphere_vertex_buffer,
            sphere_index_buffer,
            sphere_index_count,
            sphere_instance_buffer,
            sphere_instance_count,
            // UI Controls state
            paused: false,
            gravity: GRAVITATIONAL_CONSTANT,
            time_scale: 1.0,
            initial_speed: INITIAL_SPEED,
            initial_particles: initial_particles.clone(),
            initial_particle_count,
            // CPU-side particle positions for picking
            particle_positions: initial_particles
                .iter()
                .map(|p| Vec3::new(p.position[0], p.position[1], p.position[2]))
                .collect(),
            // Particle interaction
            hovered_particle_idx: None,
            dragged_particle_idx: None,
            drag_target: None,
            point_bind_group,
        }
    }

    /// Reconfigures the surface with new dimensions.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = (width, height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.projection.resize(width, height);
            // Recreate depth texture with new dimensions
            self.depth_texture = DepthTexture::new(&self.device, width, height);
        }
    }

    /// Spawns a new particle at the given world position.
    pub(crate) fn spawn_particle(&mut self, world_pos: Vec3) {
        if self.active_particle_count >= self.max_particles {
            return; // Buffer full
        }

        let particle = Particle {
            position: [world_pos.x, world_pos.y, world_pos.z],
            size: PARTICLE_SIZE,
            velocity: [0.0, 0.0, 0.0],
            _padding: 0.0,
        };

        // Write to both buffers at active_particle_count offset
        let offset = (self.active_particle_count as usize) * std::mem::size_of::<Particle>();
        self.queue.write_buffer(
            &self.particle_buffers[0],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );
        self.queue.write_buffer(
            &self.particle_buffers[1],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );

        self.active_particle_count += 1;
        self.particle_positions.push(world_pos);
    }

    /// Finds the nearest particle hit by a ray from the given screen coordinates.
    /// Returns the particle index, or None if no particle was hit.
    pub(crate) fn pick_particle(&self, screen_x: f32, screen_y: f32) -> Option<u32> {
        let (width, height) = self.size;
        let (ray_origin, ray_dir) = screen_to_world_ray(
            screen_x, screen_y, width, height, &self.camera, &self.projection,
        );

        let pick_radius = PARTICLE_SIZE * 2.0;
        let mut best_dist = f32::MAX;
        let mut best_idx: Option<u32> = None;

        for (i, pos) in self.particle_positions.iter().enumerate() {
            if let Some(dist) = ray_sphere_intersection(ray_origin, ray_dir, *pos, pick_radius) {
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(i as u32);
                }
            }
        }

        best_idx
    }

    /// Deletes a particle by swapping with the last particle (O(1)).
    pub(crate) fn delete_particle(&mut self, idx: u32) {
        if self.active_particle_count == 0 || idx >= self.active_particle_count {
            return;
        }

        let last_idx = self.active_particle_count - 1;

        if idx < last_idx {
            // Copy last particle to deleted slot in both GPU buffers
            let particle_size = std::mem::size_of::<Particle>() as u64;
            let src_offset = last_idx as u64 * particle_size;
            let dst_offset = idx as u64 * particle_size;

            let mut encoder = self.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("Delete Particle Encoder"),
                },
            );

            encoder.copy_buffer_to_buffer(
                &self.particle_buffers[0], src_offset,
                &self.particle_buffers[0], dst_offset,
                particle_size,
            );
            encoder.copy_buffer_to_buffer(
                &self.particle_buffers[1], src_offset,
                &self.particle_buffers[1], dst_offset,
                particle_size,
            );

            self.queue.submit(std::iter::once(encoder.finish()));

            // Update CPU positions: swap last into deleted slot
            self.particle_positions[idx as usize] = self.particle_positions[last_idx as usize];
        }

        self.particle_positions.pop();
        self.active_particle_count -= 1;
    }

    /// Directly sets a particle's position (used when paused + dragging).
    pub(crate) fn update_particle_position_direct(&mut self, idx: u32, pos: Vec3) {
        if idx >= self.active_particle_count {
            return;
        }

        self.particle_positions[idx as usize] = pos;

        // Create particle data with new position, zero velocity
        let particle = Particle {
            position: [pos.x, pos.y, pos.z],
            size: PARTICLE_SIZE,
            velocity: [0.0, 0.0, 0.0],
            _padding: 0.0,
        };

        let offset = (idx as usize) * std::mem::size_of::<Particle>();
        self.queue.write_buffer(
            &self.particle_buffers[0],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );
        self.queue.write_buffer(
            &self.particle_buffers[1],
            offset as u64,
            bytemuck::bytes_of(&particle),
        );
    }

    /// Renders a single frame.
    pub(crate) fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Calculate delta time
        let now = web_sys::window()
            .expect("No window")
            .performance()
            .expect("No performance")
            .now();
        let dt = match self.last_frame_time {
            Some(last) => ((now - last) / 1000.0) as f32, // ms to seconds
            None => 1.0 / 60.0, // Assume 60fps for first frame
        };
        self.last_frame_time = Some(now);
        // Clamp dt to avoid instability on lag spikes
        let dt = dt.min(1.0 / 30.0);

        // Apply time scale and pause state
        let effective_dt = if self.paused { 0.0 } else { dt * self.time_scale };

        // Update simulation parameters with user-controllable values and drag/hover state
        let (drag_idx, drag_str, drag_tgt) = match (self.dragged_particle_idx, self.drag_target) {
            (Some(idx), Some(pos)) => (idx as i32, DRAG_STRENGTH, [pos.x, pos.y, pos.z]),
            _ => (-1_i32, 0.0, [0.0; 3]),
        };
        let hover_idx = self.hovered_particle_idx.map(|i| i as i32).unwrap_or(-1);

        let sim_params = SimParams {
            delta_time: effective_dt,
            gravitational_constant: self.gravity,
            damping: DAMPING,
            particle_count: self.active_particle_count,
            softening_sq: SOFTENING_SQ,
            drag_strength: drag_str,
            dragged_particle_idx: drag_idx,
            hovered_particle_idx: hover_idx,
            drag_target: drag_tgt,
            restitution: RESTITUTION,
            floor_y: FLOOR_Y,
            cell_size: CELL_SIZE,
            num_cells: NUM_CELLS,
            _padding: 0.0,
        };
        self.queue.write_buffer(
            &self.sim_params_buffer,
            0,
            bytemuck::cast_slice(&[sim_params]),
        );

        // Update camera uniform buffer
        self.camera_uniform.update(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Determine which buffers to use for ping-pong
        let read_idx = (self.frame_index % 2) as usize;
        let write_idx = ((self.frame_index + 1) % 2) as usize;

        // Compute passes: spatial hash build + physics (skip when paused)
        if !self.paused {
            let particle_workgroups = (self.active_particle_count + 255) / 256;
            let hash_workgroups = (NUM_CELLS + 255) / 256;

            // Pass 1: Clear spatial hash
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Hash Clear Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.hash_clear_pipeline);
                pass.set_bind_group(0, &self.compute_bind_groups[read_idx], &[]);
                pass.set_bind_group(1, &self.hash_bind_group, &[]);
                pass.dispatch_workgroups(hash_workgroups, 1, 1);
            }

            // Pass 2: Insert particles into hash
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Hash Insert Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.hash_insert_pipeline);
                pass.set_bind_group(0, &self.compute_bind_groups[read_idx], &[]);
                pass.set_bind_group(1, &self.hash_bind_group, &[]);
                pass.dispatch_workgroups(particle_workgroups, 1, 1);
            }

            // Pass 3: N-body gravity + hash-based collision + integration
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Physics Compute Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.compute_pipeline);
                pass.set_bind_group(0, &self.compute_bind_groups[read_idx], &[]);
                pass.set_bind_group(1, &self.hash_bind_group, &[]);
                pass.dispatch_workgroups(particle_workgroups, 1, 1);
            }
        }
        // Implicit memory barrier here - wgpu ensures compute writes complete before render reads

        // Render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Draw lines (coordinate axes)
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.line_vertex_buffer.slice(..));
            render_pass.draw(0..self.line_vertex_count, 0..1);

            // Draw particles (instanced quads)
            // When paused, read from read_idx (no compute ran); otherwise read from write_idx
            let particle_buffer_idx = if self.paused { read_idx } else { write_idx };
            render_pass.set_pipeline(&self.point_pipeline);
            render_pass.set_bind_group(0, &self.point_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.particle_buffers[particle_buffer_idx].slice(..));
            render_pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..self.active_particle_count);

            // Draw spheres (instanced icosphere)
            render_pass.set_pipeline(&self.sphere_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.sphere_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.sphere_instance_buffer.slice(..));
            render_pass.set_index_buffer(self.sphere_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.sphere_index_count, 0, 0..self.sphere_instance_count);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Increment frame index for next frame's ping-pong (only when not paused)
        if !self.paused {
            self.frame_index = self.frame_index.wrapping_add(1);
        }

        Ok(())
    }

    /// Update camera from input deltas.
    pub(crate) fn update_camera(&mut self, rotate: (f32, f32), zoom: f32) {
        const ROTATE_SPEED: f32 = 0.005;
        const ZOOM_SPEED: f32 = 0.5;

        self.camera.rotate_azimuth(-rotate.0 * ROTATE_SPEED);
        self.camera.rotate_polar(-rotate.1 * ROTATE_SPEED);
        self.camera.zoom(zoom * ZOOM_SPEED);
    }

    /// Resets the simulation, optionally with a new particle count.
    pub(crate) fn reset_simulation(&mut self, count: Option<u32>) {
        let particles = if let Some(c) = count {
            let c = c.min(self.max_particles);
            let new_particles = create_initial_particles(c, self.initial_speed);
            self.initial_particles = new_particles.clone();
            self.initial_particle_count = c;
            new_particles
        } else {
            self.initial_particles.clone()
        };

        // Write particles to both buffers
        self.queue.write_buffer(
            &self.particle_buffers[0],
            0,
            bytemuck::cast_slice(&particles),
        );
        self.queue.write_buffer(
            &self.particle_buffers[1],
            0,
            bytemuck::cast_slice(&particles),
        );

        // Reset particle count and frame index
        self.active_particle_count = self.initial_particle_count;
        self.frame_index = 0;

        // Reset CPU positions
        self.particle_positions = particles
            .iter()
            .map(|p| Vec3::new(p.position[0], p.position[1], p.position[2]))
            .collect();

        // Clear interaction state
        self.hovered_particle_idx = None;
        self.dragged_particle_idx = None;
        self.drag_target = None;
    }
}
