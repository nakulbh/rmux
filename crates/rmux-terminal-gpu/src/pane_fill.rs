//! G0: solid-color fill of a terminal pane via wgpu paint callback.

use eframe::egui_wgpu::{self, CallbackResources, RenderState, ScreenDescriptor, wgpu};
use egui::{PaintCallbackInfo, Rect, Ui};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FillUniform {
    color: [f32; 4],
}

struct PaneFillResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl PaneFillResources {
    fn prepare(&self, queue: &wgpu::Queue, color: [f32; 4]) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&FillUniform { color }));
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

struct PaneFillCallback {
    color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for PaneFillCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(res) = resources.get::<PaneFillResources>() {
            res.prepare(queue, self.color);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(res) = resources.get::<PaneFillResources>() {
            res.paint(render_pass);
        }
    }
}

pub(crate) fn install_resources(wgpu_render_state: &RenderState) -> Result<(), String> {
    let device = &wgpu_render_state.device;
    let target_format = wgpu_render_state.target_format;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rmux_terminal_gpu_pane_fill"),
        source: wgpu::ShaderSource::Wgsl(include_str!("pane_fill.wgsl").into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rmux_terminal_gpu_pane_fill_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rmux_terminal_gpu_pane_fill_pl"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rmux_terminal_gpu_pane_fill_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmux_terminal_gpu_pane_fill_uniform"),
        contents: bytemuck::bytes_of(&FillUniform { color: [0.0, 0.0, 0.0, 1.0] }),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rmux_terminal_gpu_pane_fill_bg"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    wgpu_render_state.renderer.write().callback_resources.insert(PaneFillResources {
        pipeline,
        bind_group,
        uniform_buffer,
    });

    Ok(())
}

pub fn paint_pane_fill(ui: &mut Ui, rect: Rect, color: [f32; 4]) {
    if !rect.is_positive() {
        return;
    }
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, PaneFillCallback { color }));
}
