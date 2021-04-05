pub mod events;
use crate::{Color, Document, EditorError, EditorState};
use document_core::Operation;
use events::{Event, Response};

pub type Callback = Box<dyn Fn(Response)>;
pub struct Dispatcher {
	callback: Callback,
}

impl Dispatcher {
	pub fn handle_event(&self, editor_state: &mut EditorState, event: &Event) -> Result<(), EditorError> {
		log::trace!("{:?}", event);

		match event {
			Event::SelectTool(tool_type) => {
				editor_state.tool_state.active_tool_type = *tool_type;
			}
			Event::SelectPrimaryColor(color) => {
				editor_state.tool_state.primary_color = *color;
			}
			Event::SelectSecondaryColor(color) => {
				editor_state.tool_state.secondary_color = *color;
			}
			Event::SwapColors => {
				std::mem::swap(&mut editor_state.tool_state.primary_color, &mut editor_state.tool_state.secondary_color);
			}
			Event::ResetColors => {
				editor_state.tool_state.primary_color = Color::BLACK;
				editor_state.tool_state.secondary_color = Color::WHITE;
			}
			Event::MouseDown(mouse_state) => {
				editor_state.tool_state.mouse_state = *mouse_state;
			}
			Event::MouseUp(mouse_state) => {
				editor_state.tool_state.mouse_state = *mouse_state;
			}
			Event::MouseMove(pos) => {
				editor_state.tool_state.mouse_state.position = *pos;
			}
			Event::KeyUp(key) => todo!(),
			Event::KeyDown(key) => todo!(),
		}

		let (responses, operations) = editor_state.tool_state.active_tool()?.handle_input(event, &editor_state.document);

		self.dispatch_operations(&mut editor_state.document, &operations);
		// TODO - Dispatch Responses

		Ok(())
	}

	fn dispatch_operations(&self, document: &mut Document, operations: &[Operation]) {
		for operation in operations {
			self.dispatch_operation(document, operation);
		}
	}

	fn dispatch_operation(&self, document: &mut Document, operation: &Operation) {
		document.handle_operation(operation, |svg: String| {
			self.dispatch_response(Response::UpdateCanvas { document: svg });
		});
	}

	pub fn dispatch_responses(&self, responses: &[Response]) {
		for response in responses {
			// TODO - Remove clone when Response is Copy
			self.dispatch_response(response.clone());
		}
	}

	pub fn dispatch_response(&self, response: Response) {
		let func = &self.callback;
		// TODO - Remove clone if possible
		func(response)
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher { callback }
	}
}
