use graphene_std::raster::color::Color;
use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

pub(crate) struct GlobalEyedropperImpl {
	primary: bool,
}

impl super::NativeEyedropper for GlobalEyedropperImpl {
	fn new() -> Self {
		Self { primary: true }
	}

	fn start(&mut self, _event_loop: &dyn ActiveEventLoop, _primary: bool) {
		tracing::warn!("Global eyedropper is not yet supported on this platform");
	}

	fn stop(&mut self) {}

	fn is_active(&self) -> bool {
		false
	}

	fn window_id(&self) -> Option<WindowId> {
		None
	}

	fn update(&mut self, _position: PhysicalPosition<f64>) {}

	fn render(&self) {}

	fn sample_color(&self) -> Option<Color> {
		None
	}

	fn is_primary(&self) -> bool {
		self.primary
	}
}
