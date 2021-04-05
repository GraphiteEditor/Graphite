use crate::events::{Event, Response};
use crate::tools::Tool;
use crate::Document;
use document_core::Operation;

#[derive(Default)]
pub struct Rectangle;

impl Tool for Rectangle {
	fn handle_input(&mut self, event: &Event, document: &Document) -> (Vec<Response>, Vec<Operation>) {
		todo!();
	}
}

enum RectangleToolState {
	Ready,
	Dragging,
}
