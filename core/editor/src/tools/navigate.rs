use crate::events::Event;
use crate::tools::Tool;

pub struct Navigate;

impl Tool for Navigate {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Navigate {
	fn default() -> Self {
		Self
	}
}
