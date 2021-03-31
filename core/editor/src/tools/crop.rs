use crate::events::Event;
use crate::tools::Tool;

pub struct Crop;

impl Tool for Crop {
	fn handle_input(&mut self, event: Event) {
		todo!();
	}
}

impl Default for Crop {
	fn default() -> Self {
		Self
	}
}
