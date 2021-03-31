use crate::events::Event;
use crate::tools::Tool;
use document_core::Operation;

#[derive(Default)]
pub struct Path;

impl Tool for Path {
	fn handle_input(&mut self, event: Event) -> Option<Operation> {
		todo!();
	}
}
