use crate::events::Event;
use crate::tools::Tool;
use document_core::Operation;

#[derive(Default)]
pub struct Line;

impl Tool for Line {
	fn handle_input(&mut self, event: Event) -> Option<Operation> {
		todo!();
	}
}
