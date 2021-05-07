use super::{input_manager::InputManager, Event, EventHandler, Operation, Response};
use crate::tools::{DocumentToolData, ToolData, ToolSettings};
use document_core::document::Document;

pub struct GlobalEventHandler {}

impl GlobalEventHandler {
	fn new(tool_data: ToolData) -> Self {
		Self {}
	}

	fn pre_process_event(&mut self, input: &InputManager, events: &mut Vec<Event>, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		false
	}
}
