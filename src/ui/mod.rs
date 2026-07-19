//! egui-driven UI overlays: HUD, search, inspector, settings, minimap, and POI labels.

pub mod hud;
pub mod inspect;
pub mod minimap;
pub mod poi_labels;
pub mod search;
pub mod settings;

/// Owns the egui context, winit integration state, and wgpu renderer used to
/// draw every overlay in a frame.
pub struct EguiState {
    pub context: egui::Context,
    pub winit_state: egui_winit::State,
    pub renderer: egui_wgpu::Renderer,
}

impl EguiState {
    /// Constructs a new egui state bound to `window` and configured to paint
    /// onto a surface with the given `surface_config` format.
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        window: &winit::window::Window,
    ) -> Self {
        let context = egui::Context::default();
        let winit_state = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            None,
            None,
            None,
        );
        let renderer = egui_wgpu::Renderer::new(
            device,
            surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );
        Self {
            context,
            winit_state,
            renderer,
        }
    }
}
