use super::{Event, EventHandler, Operation, Response};
use crate::tools::{DocumentToolData, ToolData};
use crate::Document;

pub struct DocumentEventHandler {}

impl DocumentEventHandler {
	fn pre_process_event(&mut self, editor_state: &Document, tool_data: &mut DocumentToolData, events: &mut Vec<Event>, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) {}
}
