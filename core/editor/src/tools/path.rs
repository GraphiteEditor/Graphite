use crate::events::{Event, ToolResponse};
use crate::tools::Tool;
use crate::Document;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Path;

impl Tool for Path {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<ToolResponse>, Vec<Operation>) {
		todo!("{}::handle_input {:?} {:?} {:?}", module_path!(), event, document, tool_data)
	}
}
