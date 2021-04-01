use crate::events::Event;
use crate::tools::Tool;
use document_core::Operation;

#[derive(Default)]
pub struct Navigate;

impl Tool for Navigate {
	fn handle_input(&mut self, event: Event) -> Vec<Operation> {
		todo!();
	}
}
