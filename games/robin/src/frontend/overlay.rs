//! What the host draws over the game at the window's real resolution: the
//! aids panel and the picker (#92, #95), as starquake-recompiled draws its
//! guidance panel (`REUSED.md`).
//!
//! `pixels` scales the game's small buffer up to the window, which is right
//! for the Spectrum's picture and wrong for text. So the panel and picker are
//! drawn into their own RGBA texture, the size of the rectangle the picture
//! occupies on screen, and laid over it in a second pass. The texture is
//! uploaded only when its contents have changed.

use pixels::wgpu;
use pixels::wgpu::util::DeviceExt;

use super::text::Canvas;

struct Target {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

pub struct Overlay {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertices: wgpu::Buffer,
    target: Option<Target>,
    pixels: Vec<u8>,
    size: (u32, u32),
    dirty: bool,
}

impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Overlay {
        let module = device.create_shader_module(wgpu::include_wgsl!("overlay.wgsl"));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("overlay_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..wgpu::SamplerDescriptor::default()
        });
        // One triangle that covers the whole viewport.
        let corners: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
        let bytes: Vec<u8> = corners.iter().flat_map(|c| c.to_le_bytes()).collect();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("overlay_vertices"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Overlay {
            pipeline,
            layout,
            sampler,
            vertices,
            target: None,
            pixels: Vec::new(),
            size: (0, 0),
            dirty: false,
        }
    }

    /// A canvas `width` × `height` device pixels, cleared to transparent, to
    /// draw the overlay's new contents into.
    pub fn canvas(&mut self, width: u32, height: u32, scale: f32) -> Canvas<'_> {
        self.size = (width.max(1), height.max(1));
        self.pixels
            .resize(self.size.0 as usize * self.size.1 as usize * 4, 0);
        self.dirty = true;
        let mut canvas = Canvas {
            pixels: &mut self.pixels,
            width: self.size.0 as usize,
            height: self.size.1 as usize,
            scale,
        };
        canvas.clear_transparent();
        canvas
    }

    /// Lays the overlay over `view` inside `clip`, the rectangle (x, y,
    /// width, height) the game picture was drawn in.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        clip: (u32, u32, u32, u32),
    ) {
        if self.pixels.is_empty() {
            return;
        }
        if self.target.as_ref().is_none_or(|t| t.size != self.size) {
            self.target = Some(self.create_target(device));
            self.dirty = true;
        }
        let Some(target) = &self.target else {
            return;
        };
        if self.dirty {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &target.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * self.size.0),
                    rows_per_image: Some(self.size.1),
                },
                extent(self.size),
            );
            self.dirty = false;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let (x, y, w, h) = clip;
        pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &target.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..3, 0..1);
    }

    fn create_target(&self, device: &wgpu::Device) -> Target {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overlay_texture"),
            size: extent(self.size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("overlay_bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Target {
            texture,
            bind_group,
            size: self.size,
        }
    }
}

fn extent((width, height): (u32, u32)) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
