use crate::events::Event;
use crate::tools::Tool;

pub struct Sample;

impl Tool for Sample {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Sample {
	fn default() -> Self {
		Self
	}
}
