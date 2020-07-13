mod application;
mod color;
mod color_palette;
mod draw_command;
mod gui_attributes;
mod gui_node;
mod layout_abstract_syntax;
mod layout_abstract_types;
mod layout_attribute_parser;
mod layout_engine;
mod layout_system;
mod pipeline;
mod resource_cache;
mod shader_stage;
mod texture;
mod window_dom;
mod window_events;

use application::Application;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
	// Display graphics API errors (requires Vulkan SDK is installed)
	#[cfg(feature = "debug")]
	env_logger::init();

	// Handles all window events, user input, and redraws
	let event_loop = EventLoop::new();

	// Application window in the operating system
	let window = WindowBuilder::new().with_title("Graphite").build(&event_loop).unwrap();

	// Initialize the render pipeline
	let app = Application::new(&window);

	// Begin the application lifecycle
	app.begin_lifecycle(event_loop, window);
}
