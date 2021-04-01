use crate::events::Event;
use crate::tools::Tool;
use document_core::Operation;

#[derive(Default)]
pub struct Shape;

impl Tool for Shape {
	fn handle_input(&mut self, event: Event) -> Vec<Operation> {
		todo!();
	}
}
