use crate::events::Event;
use crate::tools::Tool;

pub struct Ellipse;

impl Tool for Ellipse {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Ellipse {
	fn default() -> Self {
		Self
	}
}
