use wgpu::*;

use super::AppState;
use crate::camera::{
    SHADOW_CASCADE_BLEND_DISTANCE, SHADOW_CASCADE_COUNT, SHADOW_FAR_FADE_DISTANCE, SHADOW_MAP_SIZE,
};
use crate::render::shadow_bind_group::LightUniforms;
use crate::ui::EguiState;

pub struct RenderUiState<'a> {
    pub atmosphere: &'a mut crate::atmosphere::AtmosphereSettings,
    pub day_cycle: &'a mut crate::atmosphere::DayCycleState,
    pub show_settings: &'a mut bool,
    pub minimap: &'a mut crate::ui::minimap::MinimapState,
    pub performance: &'a mut crate::app::PerformanceState,
}

pub fn render(
    state: &AppState,
    egui_state: &mut EguiState,
    screenshot_path: Option<&str>,
    ui_state: RenderUiState<'_>,
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

    let light_dir = crate::atmosphere::dominant_light_direction(ui_state.day_cycle.time_of_day);
    let cascades = state.camera.shadow_cascades(light_dir);
    let light_uniforms = LightUniforms {
        light_view_proj: cascades
            .cascades
            .map(|cascade| cascade.light_view_proj.to_cols_array_2d()),
        cascade_radii: cascades.cascades.map(|cascade| cascade.radius),
        shadow_params: [
            SHADOW_MAP_SIZE as f32,
            SHADOW_CASCADE_BLEND_DISTANCE,
            SHADOW_FAR_FADE_DISTANCE,
            0.0,
        ],
        shadow_pass_params: [
            0,
            ui_state.atmosphere.shadow_cascade_debug as u32,
            SHADOW_CASCADE_COUNT as u32,
            0,
        ],
    };
    state.shadow_bg.update(&state.queue, &light_uniforms);

    let mut shadow_encoder = state
        .device
        .create_command_encoder(&CommandEncoderDescriptor {
            label: Some("shadow render encoder"),
        });
    for (cascade_index, cascade_view) in state.shadow_bg.cascade_views.iter().enumerate() {
        let mut shadow_pass = shadow_encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("shadow render pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: cascade_view,
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
        shadow_pass.set_bind_group(0, &state.shadow_bg.pass_groups[cascade_index], &[]);
        shadow_pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
        shadow_pass.set_index_buffer(
            state.scene.shadow_index_buffer.slice(..),
            IndexFormat::Uint32,
        );
        shadow_pass.draw_indexed(0..state.scene.shadow_index_count, 0, 0..1);
    }
    state.queue.submit(std::iter::once(shadow_encoder.finish()));

    let mut encoder = state
        .device
        .create_command_encoder(&CommandEncoderDescriptor {
            label: Some("render encoder"),
        });

    // Resolve previous frame's occlusion queries
    encoder.resolve_query_set(
        &state.occlusion.query_set,
        0..state.occlusion.query_count,
        &state.occlusion.result_buffer,
        0,
    );

    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("main render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &state.contact_shadow.color_view,
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

    // Minimap pass
    if ui_state.minimap.visible {
        let minimap_uniforms = state.minimap_target.uniforms(
            &state.camera,
            ui_state.day_cycle,
            ui_state.atmosphere,
            ui_state.minimap.zoom,
            ui_state.minimap.rotate_with_camera,
        );
        state
            .minimap_target
            .bind_group
            .update(&state.queue, &minimap_uniforms);

        {
            let mut minimap_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("minimap render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &state.minimap_target.color_view,
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
                    view: &state.minimap_target.depth_view,
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

            minimap_pass.set_pipeline(&state.sky_pipeline.pipeline);
            minimap_pass.set_bind_group(0, &state.minimap_target.bind_group.group, &[]);
            minimap_pass.draw(0..3, 0..1);

            minimap_pass.set_pipeline(&state.pipeline.pipeline);
            minimap_pass.set_bind_group(0, &state.minimap_target.bind_group.group, &[]);
            minimap_pass.set_bind_group(1, &state.shadow_bg.group, &[]);
            minimap_pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
            minimap_pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
            minimap_pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
        }
    }

    {
        let mut post_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("contact shadow composite pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        post_pass.set_pipeline(&state.contact_shadow.pipeline);
        post_pass.set_bind_group(0, &state.camera_bg.group, &[]);
        post_pass.set_bind_group(1, &state.contact_shadow.bind_group, &[]);
        post_pass.draw(0..3, 0..1);
    }

    // egui pass
    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [state.surface_config.width, state.surface_config.height],
        pixels_per_point: state.window.scale_factor() as f32,
    };

    if ui_state.minimap.texture_id.is_none() {
        ui_state.minimap.texture_id = Some(egui_state.renderer.register_native_texture(
            &state.device,
            &state.minimap_target.color_view,
            wgpu::FilterMode::Linear,
        ));
    }

    let viewport_size = egui::vec2(
        state.surface_config.width as f32 / screen_descriptor.pixels_per_point,
        state.surface_config.height as f32 / screen_descriptor.pixels_per_point,
    );
    let raw_input = egui_state.winit_state.take_egui_input(&state.window);
    #[allow(deprecated)]
    let egui_output = egui_state.context.run(raw_input, |ctx| {
        let camera_lat_lon = state
            .coord_converter
            .map(|conv| conv.world_xz_to_lat_lon(state.camera.position.x, state.camera.position.z));
        crate::ui::hud::draw(
            ctx,
            &state.camera,
            camera_lat_lon,
            ui_state.day_cycle,
            ui_state.performance,
        );
        crate::ui::poi_labels::draw(ctx, &state.camera, &state.poi_labels, viewport_size);
        if *ui_state.show_settings {
            crate::ui::settings::draw(
                ctx,
                ui_state.atmosphere,
                ui_state.day_cycle,
                ui_state.performance,
                ui_state.minimap,
                ui_state.show_settings,
            );
        }
        crate::ui::minimap::draw(ctx, &state.camera, ui_state.minimap);
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
