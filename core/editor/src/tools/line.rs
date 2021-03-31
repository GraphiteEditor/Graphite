use crate::events::Event;
use crate::tools::Tool;

pub struct Line;

impl Tool for Line {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Line {
	fn default() -> Self {
		Self
	}
}
