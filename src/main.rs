mod application;
mod pipeline;
mod texture;
mod color;
mod color_palette;
mod resource_cache;
mod shader_stage;
mod draw_command;
mod gui_node;
mod gui_attributes;
mod window_events;
mod component_layout;
mod parsed_layout_node;

use application::Application;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
	// Handles all window events, user input, and redraws
	let event_loop = EventLoop::new();

	// Application window in the operating system
	let window = WindowBuilder::new().with_title("Graphite").build(&event_loop).unwrap();

	// Initialize the render pipeline
	let app = Application::new(&window);

	// Begin the application lifecycle
	app.begin_lifecycle(event_loop, window);
}
