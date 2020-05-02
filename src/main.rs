mod application;
mod gui_rect;
mod pipeline;
mod texture;
mod color_palette;
mod shader_cache;
mod pipeline_cache;
mod draw_command;

use application::Application;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
	// Handles all window events, user input, and redraws
	let event_loop = EventLoop::new();

	// Application window in the operating system
	let window = WindowBuilder::new().with_title("Graphite").build(&event_loop).unwrap();

	// Initialize the render pipeline
	let mut app = Application::new(&window);
	app.example();

	// State managers for render pipeline and program logic
	// let app_render_state = RenderState::new(&mut app);

	// Begin the application lifecycle
	app.begin_lifecycle(event_loop, window);
}
