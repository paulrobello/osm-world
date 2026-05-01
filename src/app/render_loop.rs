use wgpu::*;

use super::AppState;

pub fn render(state: &AppState) {
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

    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("main render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.53,
                        g: 0.81,
                        b: 0.92,
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

        pass.set_pipeline(&state.pipeline.pipeline);
        pass.set_bind_group(0, &state.camera_bg.group, &[]);
        pass.set_vertex_buffer(0, state.scene.vertex_buffer.slice(..));
        pass.set_index_buffer(state.scene.index_buffer.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(0..state.scene.index_count, 0, 0..1);
    }

    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
