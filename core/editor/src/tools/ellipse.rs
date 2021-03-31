use crate::events::Event;
use crate::tools::Tool;
use document_core::Operation;

#[derive(Default)]
pub struct Ellipse;

impl Tool for Ellipse {
	fn handle_input(&mut self, event: Event) -> Option<Operation> {
		todo!();
	}
}
