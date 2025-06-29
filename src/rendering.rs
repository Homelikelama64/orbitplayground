use eframe::{egui, wgpu};
use encase::{ShaderSize, ShaderType};

#[derive(ShaderType)]
pub struct GpuCamera {
    pub position: cgmath::Vector2<f32>,
    pub vertical_height: f32,
    pub aspect: f32,
}

#[derive(ShaderType)]
pub struct GpuQuad {
    pub position: cgmath::Vector3<f32>,
    pub rotation: f32,
    pub color: cgmath::Vector3<f32>,
    pub size: cgmath::Vector2<f32>,
}

#[derive(ShaderType)]
pub struct GpuCircle {
    pub position: cgmath::Vector3<f32>,
    pub color: cgmath::Vector3<f32>,
    pub radius: f32,
}

pub struct RenderState {
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    quads_buffer: wgpu::Buffer,
    quads_bind_group_layout: wgpu::BindGroupLayout,
    quads_bind_group: wgpu::BindGroup,

    quad_render_pipeline: wgpu::RenderPipeline,

    circles_buffer: wgpu::Buffer,
    circles_bind_group_layout: wgpu::BindGroupLayout,
    circles_bind_group: wgpu::BindGroup,

    circle_render_pipeline: wgpu::RenderPipeline,
}

impl RenderState {
    pub fn new(
        target_format: wgpu::TextureFormat,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> anyhow::Result<Self> {
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: GpuCamera::SHADER_SIZE.get(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(GpuCamera::SHADER_SIZE),
                    },
                    count: None,
                }],
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let quads_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Quads Buffer"),
            size: GpuQuad::SHADER_SIZE.get(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quads_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Quads Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(GpuQuad::SHADER_SIZE),
                    },
                    count: None,
                }],
            });
        let quads_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Quads Bind Group"),
            layout: &quads_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: quads_buffer.as_entire_binding(),
            }],
        });

        let quad_shader = device.create_shader_module(wgpu::include_wgsl!("./quad_shader.wgsl"));

        let quad_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Quad Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &quads_bind_group_layout],
                push_constant_ranges: &[],
            });
        let quad_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Quad Render Pipeline"),
            layout: Some(&quad_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &quad_shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let circles_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Circles Buffer"),
            size: GpuCircle::SHADER_SIZE.get(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let circles_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Circles Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(GpuCircle::SHADER_SIZE),
                    },
                    count: None,
                }],
            });
        let circles_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Circles Bind Group"),
            layout: &circles_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: circles_buffer.as_entire_binding(),
            }],
        });

        let circle_shader =
            device.create_shader_module(wgpu::include_wgsl!("./circle_shader.wgsl"));

        let circle_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Circle Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &circles_bind_group_layout],
                push_constant_ranges: &[],
            });
        let circle_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Circle Render Pipeline"),
                layout: Some(&circle_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &circle_shader,
                    entry_point: Some("vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &circle_shader,
                    entry_point: Some("fragment"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            });

        Ok(Self {
            camera_buffer,
            camera_bind_group,

            quads_buffer,
            quads_bind_group_layout,
            quads_bind_group,

            quad_render_pipeline,

            circles_buffer,
            circles_bind_group_layout,
            circles_bind_group,

            circle_render_pipeline,
        })
    }
}

pub struct RenderData {
    pub camera: GpuCamera,
    pub quads: Vec<GpuQuad>,
    pub circles: Vec<GpuCircle>,
}

impl eframe::egui_wgpu::CallbackTrait for RenderData {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &eframe::egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let state: &mut RenderState = callback_resources.get_mut().unwrap();

        {
            let mut camera_buffer = queue
                .write_buffer_with(&state.camera_buffer, 0, GpuCamera::SHADER_SIZE)
                .unwrap();
            encase::UniformBuffer::new(&mut *camera_buffer)
                .write(&self.camera)
                .unwrap();
        }

        {
            let size = self.quads.size();
            if size.get() > state.quads_buffer.size() {
                state.quads_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Quads Buffer"),
                    size: size.get(),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                state.quads_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Quads Bind Group"),
                    layout: &state.quads_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state.quads_buffer.as_entire_binding(),
                    }],
                });
            }

            let mut quads_buffer = queue
                .write_buffer_with(&state.quads_buffer, 0, size)
                .unwrap();
            encase::StorageBuffer::new(&mut *quads_buffer)
                .write(&self.quads)
                .unwrap();
        }

        {
            let size = self.circles.size();
            if size.get() > state.circles_buffer.size() {
                state.circles_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Circles Buffer"),
                    size: size.get(),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                state.circles_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Circles Bind Group"),
                    layout: &state.circles_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state.circles_buffer.as_entire_binding(),
                    }],
                });
            }

            let mut circles_buffer = queue
                .write_buffer_with(&state.circles_buffer, 0, size)
                .unwrap();
            encase::StorageBuffer::new(&mut *circles_buffer)
                .write(&self.circles)
                .unwrap();
        }

        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &eframe::egui_wgpu::CallbackResources,
    ) {
        let state: &RenderState = callback_resources.get().unwrap();

        render_pass.set_pipeline(&state.quad_render_pipeline);
        render_pass.set_bind_group(0, &state.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &state.quads_bind_group, &[]);
        render_pass.draw(0..4, 0..self.quads.len() as _);

        render_pass.set_pipeline(&state.circle_render_pipeline);
        render_pass.set_bind_group(0, &state.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &state.circles_bind_group, &[]);
        render_pass.draw(0..4, 0..self.circles.len() as _);
    }
}
