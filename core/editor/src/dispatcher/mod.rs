pub mod events;
use crate::{tools::ToolType, Color, Document, EditorError, EditorState};
use document_core::Operation;
use events::{Event, Key, Response};

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
				editor_state.tool_state.swap_colors();
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
			Event::KeyUp(key) => (),
			Event::KeyDown(key) => {
				log::trace!("pressed key {:?}", key);
				log::debug!("pressed key {:?}", key);

				match key {
					Key::Key0 => {
						log::set_max_level(log::LevelFilter::Info);
						log::debug!("set log verbosity to info");
					}
					Key::Key1 => {
						log::set_max_level(log::LevelFilter::Debug);
						log::debug!("set log verbosity to debug");
					}
					Key::Key2 => {
						log::set_max_level(log::LevelFilter::Trace);
						log::debug!("set log verbosity to trace");
					}
					Key::KeyM => {
						editor_state.tool_state.active_tool_type = ToolType::Rectangle;
					}
					Key::KeyE => {
						editor_state.tool_state.active_tool_type = ToolType::Ellipse;
					}
					Key::KeyV => {
						editor_state.tool_state.active_tool_type = ToolType::Select;
					}
					Key::KeyX => {
						editor_state.tool_state.swap_colors();
					}
					_ => (),
				}
			}
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
