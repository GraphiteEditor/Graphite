use crate::events::{Event, Response};
use crate::tools::Tool;
use crate::Document;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Eyedropper;

impl Tool for Eyedropper {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<Response>, Vec<Operation>) {
		todo!("{}::handle_input {:?} {:?} {:?}", module_path!(), event, document, tool_data)
	}
}
