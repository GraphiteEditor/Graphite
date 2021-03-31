use crate::events::Event;
use crate::tools::Tool;

pub struct Pen;

impl Tool for Pen {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Pen {
	fn default() -> Self {
		Self
	}
}
