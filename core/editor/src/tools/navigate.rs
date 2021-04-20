use crate::events::{Event, Response};
use crate::tools::Tool;
use crate::Document;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Navigate;

impl Tool for Navigate {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<Response>, Vec<Operation>) {
		todo!();
	}
}
