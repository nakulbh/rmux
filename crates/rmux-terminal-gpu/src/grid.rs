//! G2: full-grid GPU paint from [`rmux_terminal::GridSnapshot`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use eframe::egui_wgpu::{self, CallbackResources, RenderState, ScreenDescriptor, wgpu};
use egui::{Color32, PaintCallbackInfo, Rect, Ui};
use rmux_terminal::GridSnapshot;

use crate::atlas::GlyphAtlas;

const MAX_INSTANCES: u64 = 64 * 1024;

static NEXT_PANE_ID: AtomicU64 = AtomicU64::new(1);
static GRID_READY: AtomicBool = AtomicBool::new(false);
static FRAME_EPOCH: AtomicU64 = AtomicU64::new(1);

/// Allocate a stable id for multipane instance-buffer ranges.
pub fn alloc_pane_id() -> u64 {
    NEXT_PANE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Whether the full-grid GPU pipeline is installed (not just G0 fill).
#[inline]
pub fn grid_is_ready() -> bool {
    GRID_READY.load(Ordering::Relaxed)
}

pub(crate) fn set_grid_ready(ready: bool) {
    GRID_READY.store(ready, Ordering::Relaxed);
}

/// Begin a new UI frame’s GPU terminal paints (call once per `App::update`).
pub fn begin_frame() {
    FRAME_EPOCH.fetch_add(1, Ordering::Relaxed);
}

fn current_epoch() -> u64 {
    FRAME_EPOCH.load(Ordering::Relaxed)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CellInstance {
    geo: [f32; 4],     // col, row, span, flags
    metrics: [f32; 4], // pane_w, pane_h, cell_w, cell_h
    fg: [f32; 4],
    bg: [f32; 4],
    glyph_rect: [f32; 4], // ox, oy, gw, gh
    uv_min: [f32; 2],
    uv_max: [f32; 2],
}

#[derive(Clone, Copy)]
struct CpuCell {
    col: u16,
    row: u16,
    span: u16,
    c: char,
    bold: bool,
    underline: bool,
    cursor: bool,
    fg: [f32; 4],
    bg: [f32; 4],
}

pub(crate) struct GridGpu {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    atlas_texture: wgpu::Texture,
    atlas: GlyphAtlas,
    ranges: HashMap<u64, (u32, u32)>,
    next_instance: u32,
    last_prepare_epoch: u64,
}

impl GridGpu {
    pub fn install(
        wgpu_render_state: &RenderState,
        font_regular: &[u8],
        font_bold: &[u8],
        font_size: f32,
    ) -> Result<(), String> {
        let device = &wgpu_render_state.device;
        let queue = &wgpu_render_state.queue;
        let target_format = wgpu_render_state.target_format;

        let atlas = GlyphAtlas::new(font_regular, font_bold, font_size)?;
        let (aw, ah) = atlas.dimensions();

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rmux_term_atlas"),
            size: wgpu::Extent3d { width: aw, height: ah, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rmux_term_atlas_samp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            atlas.rgba(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aw * 4),
                rows_per_image: Some(ah),
            },
            wgpu::Extent3d { width: aw, height: ah, depth_or_array_layers: 1 },
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rmux_term_grid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grid.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rmux_term_grid_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rmux_term_grid_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rmux_term_grid_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_stride = std::mem::size_of::<CellInstance>() as u64;
        const INSTANCE_ATTRS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x4,
            5 => Float32x2,
            6 => Float32x2,
        ];
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: instance_stride,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &INSTANCE_ATTRS,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rmux_term_grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // Match egui's default mesh blending.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rmux_term_instances"),
            size: instance_stride * MAX_INSTANCES,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        wgpu_render_state.renderer.write().callback_resources.insert(GridGpu {
            pipeline,
            bind_group,
            instance_buffer,
            atlas_texture,
            atlas,
            ranges: HashMap::new(),
            next_instance: 0,
            last_prepare_epoch: 0,
        });

        set_grid_ready(true);
        Ok(())
    }

    fn begin_frame_if_needed(&mut self, epoch: u64) {
        if self.last_prepare_epoch != epoch {
            self.last_prepare_epoch = epoch;
            self.next_instance = 0;
            self.ranges.clear();
        }
    }

    fn upload_atlas_if_dirty(&mut self, queue: &wgpu::Queue) {
        if !self.atlas.dirty {
            return;
        }
        let (w, h) = self.atlas.dimensions();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            self.atlas.rgba(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.atlas.dirty = false;
    }
}

struct GridPaintCallback {
    pane_id: u64,
    font_size: f32,
    pane_w: f32,
    pane_h: f32,
    cells: Vec<CpuCell>,
    frame_epoch: u64,
}

impl egui_wgpu::CallbackTrait for GridPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<GridGpu>() else {
            tracing::warn!("rmux-terminal-gpu: GridGpu missing in prepare");
            return Vec::new();
        };

        gpu.begin_frame_if_needed(self.frame_epoch);
        gpu.atlas.set_font_size(self.font_size);

        let cell_w = gpu.atlas.cell_w.max(1.0);
        let cell_h = gpu.atlas.cell_h.max(1.0);
        let metrics = [self.pane_w, self.pane_h, cell_w, cell_h];

        let mut instances = Vec::with_capacity(self.cells.len());
        for cell in &self.cells {
            let g = gpu.atlas.glyph(cell.c, cell.bold);
            let mut flags = 0.0_f32;
            if cell.underline {
                flags += 1.0;
            }
            if cell.cursor {
                flags += 2.0;
            }

            // Keep real placement (may extend slightly outside the cell);
            // the shader samples only where glyph_local is inside 0..1.
            let (glyph_rect, uv_min, uv_max) = if g.has_ink {
                ([g.ox, g.oy, g.gw.max(1.0), g.gh.max(1.0)], g.uv_min, g.uv_max)
            } else {
                ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0], [0.0, 0.0])
            };

            instances.push(CellInstance {
                geo: [f32::from(cell.col), f32::from(cell.row), f32::from(cell.span), flags],
                metrics,
                fg: cell.fg,
                bg: cell.bg,
                glyph_rect,
                uv_min,
                uv_max,
            });
        }

        gpu.upload_atlas_if_dirty(queue);

        let count = instances.len() as u32;
        if count == 0 {
            gpu.ranges.insert(self.pane_id, (0, 0));
            return Vec::new();
        }

        let first = gpu.next_instance;
        if u64::from(first) + u64::from(count) > MAX_INSTANCES {
            tracing::warn!(
                first,
                count,
                "terminal GPU instance buffer full; skipping pane {}",
                self.pane_id
            );
            gpu.ranges.insert(self.pane_id, (0, 0));
            return Vec::new();
        }

        let byte_offset = u64::from(first) * std::mem::size_of::<CellInstance>() as u64;
        queue.write_buffer(&gpu.instance_buffer, byte_offset, bytemuck::cast_slice(&instances));
        gpu.next_instance = first + count;
        gpu.ranges.insert(self.pane_id, (first, count));
        Vec::new()
    }

    fn paint(
        &self,
        info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(gpu) = resources.get::<GridGpu>() else {
            return;
        };
        let Some(&(first, count)) = gpu.ranges.get(&self.pane_id) else {
            return;
        };
        if count == 0 {
            return;
        }

        // Match scissor to the pane viewport so we are not clipped to a
        // previous mesh's scissor rect from the egui batcher.
        let vp = info.viewport_in_pixels();
        if vp.width_px <= 0 || vp.height_px <= 0 {
            return;
        }
        let x = vp.left_px.max(0) as u32;
        let y = vp.top_px.max(0) as u32;
        let w = vp.width_px as u32;
        let h = vp.height_px as u32;
        render_pass.set_scissor_rect(x, y, w, h);
        render_pass.set_viewport(
            vp.left_px as f32,
            vp.top_px as f32,
            vp.width_px as f32,
            vp.height_px as f32,
            0.0,
            1.0,
        );

        let stride = std::mem::size_of::<CellInstance>() as u64;
        let start = u64::from(first) * stride;
        let end = start + u64::from(count) * stride;

        render_pass.set_pipeline(&gpu.pipeline);
        render_pass.set_bind_group(0, &gpu.bind_group, &[]);
        render_pass.set_vertex_buffer(0, gpu.instance_buffer.slice(start..end));
        render_pass.draw(0..6, 0..count);
    }
}

/// Paint the terminal grid with the GPU path.
///
/// Returns `false` if the grid pipeline is not installed (caller must use egui).
pub fn paint_grid(
    ui: &mut Ui,
    rect: Rect,
    snapshot: &GridSnapshot,
    cursor_visible: bool,
    opacity: f32,
    font_size: f32,
    pane_id: u64,
) -> bool {
    if !grid_is_ready() || !rect.is_positive() {
        return false;
    }

    let visible_cols = snapshot.cols;
    let visible_rows = snapshot.rows;
    if visible_cols == 0 || visible_rows == 0 {
        return false;
    }

    let opacity = opacity.clamp(0.0, 1.0);
    // Keep terminal glass readable: never fully disappear into wallpaper.
    let bg_opacity = opacity.max(0.35);
    let term_bg = snapshot.terminal_bg;
    let mut cells = Vec::with_capacity(visible_cols as usize * visible_rows as usize);

    for row in 0..visible_rows {
        let mut col = 0u16;
        while col < visible_cols {
            let cell = &snapshot.cells[row as usize][col as usize];
            let span = if cell.wide && col + 1 < visible_cols { 2 } else { 1 };
            let bg = if same_rgb(cell.bg, term_bg) {
                with_opacity(term_bg, bg_opacity)
            } else {
                // Custom cell backgrounds (nvim, TUIs) stay nearly solid.
                with_opacity(cell.bg, bg_opacity.max(0.92))
            };
            let cursor = cursor_visible && row == snapshot.cursor_row && col == snapshot.cursor_col;
            cells.push(CpuCell {
                col,
                row,
                span,
                c: cell.c,
                bold: cell.bold,
                underline: cell.underline,
                cursor,
                fg: rgba_straight(cell.fg),
                bg: rgba_straight(bg),
            });
            col += span;
        }
    }

    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        GridPaintCallback {
            pane_id,
            font_size,
            pane_w: rect.width().max(1.0),
            pane_h: rect.height().max(1.0),
            cells,
            frame_epoch: current_epoch(),
        },
    ));
    true
}

fn same_rgb(a: Color32, b: Color32) -> bool {
    a.r() == b.r() && a.g() == b.g() && a.b() == b.b()
}

fn with_opacity(c: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r(),
        c.g(),
        c.b(),
        (f32::from(c.a()) * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

fn rgba_straight(c: Color32) -> [f32; 4] {
    [
        f32::from(c.r()) / 255.0,
        f32::from(c.g()) / 255.0,
        f32::from(c.b()) / 255.0,
        f32::from(c.a()) / 255.0,
    ]
}
