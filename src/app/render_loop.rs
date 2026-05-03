use wgpu::*;

use super::AppState;
use crate::render::shadow_bind_group::LightUniforms;
use crate::ui::EguiState;

pub fn render(
    state: &AppState,
    egui_state: &mut EguiState,
    screenshot_path: Option<&str>,
    atmosphere: &mut crate::atmosphere::AtmosphereSettings,
    day_cycle: &mut crate::atmosphere::DayCycleState,
    show_settings: &mut bool,
) {
    let output = match state.surface.get_current_texture() {
        CurrentSurfaceTexture::Success(frame) => frame,
        CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return,
        CurrentSurfaceTexture::Outdated
        | CurrentSurfaceTexture::Suboptimal(_)
        | CurrentSurfaceTexture::Lost => {
            state
                .surface
                .configure(&state.device, &state.surface_config);
            return;
        }
        CurrentSurfaceTexture::Validation => return,
    };

    let view = output
        .texture
        .create_view(&TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&CommandEncoderDescriptor {
            label: Some("render encoder"),
        });

    // Upload light VP for shadow mapping
    let sun_dir = crate::atmosphere::sun_direction(day_cycle.time_of_day);
    let light_vp = state.camera.light_view_proj(sun_dir);
    state.shadow_bg.update(
        &state.queue,
        &LightUniforms {
            light_view_proj: light_vp.to_cols_array_2d(),
        },
    );

    // Shadow pass
    {
        let mut shadow_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("shadow render pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &state.shadow_bg.depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        shadow_pass.set_pipeline(&state.shadow_pipeline.pipeline);
        shadow_pass.set_bind_group(0, &state.shadow_bg.group, &[]);
        shadow_pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
        shadow_pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
        shadow_pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
    }

    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("main render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &state.depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Sky pass (full-screen triangle, depth LessEqual, no depth write)
        pass.set_pipeline(&state.sky_pipeline.pipeline);
        pass.set_bind_group(0, &state.camera_bg.group, &[]);
        pass.draw(0..3, 0..1);

        // City pass
        pass.set_pipeline(&state.pipeline.pipeline);
        pass.set_bind_group(0, &state.camera_bg.group, &[]);
        pass.set_bind_group(1, &state.shadow_bg.group, &[]);
        pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
        pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
    }

    // egui pass
    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [state.surface_config.width, state.surface_config.height],
        pixels_per_point: state.window.scale_factor() as f32,
    };

    let raw_input = egui_state.winit_state.take_egui_input(&state.window);
    #[allow(deprecated)]
    let egui_output = egui_state.context.run(raw_input, |ctx| {
        crate::ui::hud::draw(ctx, &state.camera, day_cycle);
        if *show_settings {
            crate::ui::settings::draw(ctx, atmosphere, day_cycle, show_settings);
        }
    });

    egui_state
        .winit_state
        .handle_platform_output(&state.window, egui_output.platform_output);

    let tris = egui_state
        .context
        .tessellate(egui_output.shapes, egui_output.pixels_per_point);

    for (id, image_delta) in &egui_output.textures_delta.set {
        egui_state
            .renderer
            .update_texture(&state.device, &state.queue, *id, image_delta);
    }
    egui_state.renderer.update_buffers(
        &state.device,
        &state.queue,
        &mut encoder,
        &tris,
        &screen_descriptor,
    );

    {
        let pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("egui render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        egui_state
            .renderer
            .render(&mut pass.forget_lifetime(), &tris, &screen_descriptor);
    }

    let screenshot_buffer = if screenshot_path.is_some() {
        let width = state.surface_config.width;
        let height = state.surface_config.height;
        let unpadded_bytes_per_row = width * 4;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(256) * 256;

        let buffer = state.device.create_buffer(&BufferDescriptor {
            label: Some("screenshot buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Some((buffer, width, height, padded_bytes_per_row))
    } else {
        None
    };

    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    if let Some(path) = screenshot_path {
        if let Some((buffer, width, height, padded_bytes_per_row)) = screenshot_buffer {
            save_screenshot(state, &buffer, width, height, padded_bytes_per_row, path);
        }
    }
}

fn save_screenshot(
    state: &AppState,
    buffer: &Buffer,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
    path: &str,
) {
    let slice = buffer.slice(..);
    slice.map_async(MapMode::Read, |_| {});
    let _ = state.device.poll(PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let data = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let offset = (row * padded_bytes_per_row) as usize;
        let row_data = &data[offset..offset + (width * 4) as usize];
        pixels.extend_from_slice(row_data);
    }
    drop(data);
    buffer.unmap();

    // Swap B and R channels (BGRA -> RGBA)
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }
    if let Some(img) = image::RgbaImage::from_raw(width, height, pixels) {
        match img.save(path) {
            Ok(()) => log::info!("[SCREENSHOT] Saved {}x{} to {}", width, height, path),
            Err(e) => log::error!("[SCREENSHOT] Failed to write {}: {}", path, e),
        }
    }
}
