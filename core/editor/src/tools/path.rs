use crate::events::Event;
use crate::tools::Tool;

pub struct Path;

impl Tool for Path {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Path {
	fn default() -> Self {
		Self
	}
}
