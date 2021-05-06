use crate::events::{CanvasTransform, Event, ToolResponse};
use crate::tools::Tool;
use crate::Document;
use document_core::Operation;

use super::DocumentToolData;

#[derive(Default)]
pub struct Eyedropper;

impl Tool for Eyedropper {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData, canvas_transform: &CanvasTransform) -> (Vec<ToolResponse>, Vec<Operation>) {
		todo!("{}::handle_input {:?} {:?} {:?} {:?}", module_path!(), event, document, tool_data, canvas_transform)
	}
}
