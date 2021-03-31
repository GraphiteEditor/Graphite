use crate::events::Event;
use crate::tools::Tool;

pub struct Shape;

impl Tool for Shape {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Shape {
	fn default() -> Self {
		Self
	}
}
