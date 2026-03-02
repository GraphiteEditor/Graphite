use graphene_std::raster::color::Color;
use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

pub(crate) trait NativeEyedropper {
	fn new() -> Self;
	fn start(&mut self, event_loop: &dyn ActiveEventLoop, primary: bool);
	fn stop(&mut self);
	fn is_active(&self) -> bool;
	fn window_id(&self) -> Option<WindowId>;
	fn update(&mut self, position: PhysicalPosition<f64>);
	fn render(&self);
	fn sample_color(&self) -> Option<Color>;
	fn is_primary(&self) -> bool;
}

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win as native;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as native;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as native;

pub(crate) type GlobalEyedropper = native::GlobalEyedropperImpl;
