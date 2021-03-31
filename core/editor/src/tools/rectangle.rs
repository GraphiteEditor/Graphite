use crate::events::Event;
use crate::tools::Tool;

pub struct Rectangle;

impl Tool for Rectangle {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Rectangle {
	fn default() -> Self {
		Self
	}
}
