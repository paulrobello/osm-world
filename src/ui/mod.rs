pub mod hud;
pub mod settings;

pub struct EguiState {
    pub context: egui::Context,
    pub winit_state: egui_winit::State,
    pub renderer: egui_wgpu::Renderer,
}

impl EguiState {
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
