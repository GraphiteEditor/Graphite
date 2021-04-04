pub mod events;
use crate::{Color, Document, EditorError, ToolState};
use document_core::Operation;
use events::{Event, Response};

pub type Callback = Box<dyn Fn(Response)>;
pub struct Dispatcher {
	callback: Callback,
}

impl Dispatcher {
	pub fn handle_event(&self, tool_state: &mut ToolState, document: &mut Document, event: &Event) -> Result<(), EditorError> {
		log::trace!("{:?}", event);

		match event {
			Event::SelectTool(tool_type) => {
				tool_state.active_tool_type = *tool_type;
			}
			Event::SelectPrimaryColor(color) => {
				tool_state.primary_color = *color;
			}
			Event::SelectSecondaryColor(color) => {
				tool_state.secondary_color = *color;
			}
			Event::SwapColors => {
				std::mem::swap(&mut tool_state.primary_color, &mut tool_state.secondary_color);
			}
			Event::ResetColors => {
				tool_state.primary_color = Color::BLACK;
				tool_state.secondary_color = Color::WHITE;
			}
			Event::MouseDown(mouse_state) => {
				tool_state.mouse_state = *mouse_state;
			}
			Event::MouseUp(mouse_state) => {
				tool_state.mouse_state = *mouse_state;
			}
			Event::MouseMovement(pos) => {
				tool_state.mouse_state.position = *pos;
			}
			Event::ModifierKeyDown(mod_keys) => {
				tool_state.mod_keys = *mod_keys;
			}
			Event::ModifierKeyUp(mod_keys) => {
				tool_state.mod_keys = *mod_keys;
			}
			Event::KeyPress(key) => todo!(),
		}

		let (responses, operations) = tool_state.active_tool()?.handle_input(event, document);

		self.dispatch_operations(document, &operations);
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
			self.dispatch_response(&Response::UpdateCanvas { document: svg });
		});
	}

	pub fn dispatch_responses(&self, responses: &[Response]) {
		for response in responses {
			self.dispatch_response(response);
		}
	}

	pub fn dispatch_response(&self, response: &Response) {
		let func = &self.callback;
		// TODO - Remove clone if possible
		func(response.clone())
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher { callback }
	}
}
