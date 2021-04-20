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
			Event::SelectTool(tool_name) => {
				editor_state.tool_state.tool_data.active_tool_type = *tool_name;
				self.dispatch_response(Response::SetActiveTool { tool_name: tool_name.to_string() });
			}
			Event::SelectPrimaryColor(color) => {
				editor_state.tool_state.document_tool_data.primary_color = *color;
			}
			Event::SelectSecondaryColor(color) => {
				editor_state.tool_state.document_tool_data.secondary_color = *color;
			}
			Event::SwapColors => {
				editor_state.tool_state.swap_colors();
			}
			Event::ResetColors => {
				editor_state.tool_state.document_tool_data.primary_color = Color::BLACK;
				editor_state.tool_state.document_tool_data.secondary_color = Color::WHITE;
			}
			Event::MouseDown(mouse_state) => {
				editor_state.tool_state.document_tool_data.mouse_state = *mouse_state;
			}
			Event::MouseUp(mouse_state) => {
				editor_state.tool_state.document_tool_data.mouse_state = *mouse_state;
			}
			Event::MouseMove(pos) => {
				editor_state.tool_state.document_tool_data.mouse_state.position = *pos;
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
					Key::KeyV => {
						editor_state.tool_state.tool_data.active_tool_type = ToolType::Select;
						self.dispatch_response(Response::SetActiveTool {
							tool_name: ToolType::Select.to_string(),
						});
					}
					Key::KeyL => {
						editor_state.tool_state.tool_data.active_tool_type = ToolType::Line;
						self.dispatch_response(Response::SetActiveTool {
							tool_name: ToolType::Line.to_string(),
						});
					}
					Key::KeyM => {
						editor_state.tool_state.tool_data.active_tool_type = ToolType::Rectangle;
						self.dispatch_response(Response::SetActiveTool {
							tool_name: ToolType::Rectangle.to_string(),
						});
					}
					Key::KeyY => {
						editor_state.tool_state.tool_data.active_tool_type = ToolType::Shape;
						self.dispatch_response(Response::SetActiveTool {
							tool_name: ToolType::Shape.to_string(),
						});
					}
					Key::KeyE => {
						editor_state.tool_state.tool_data.active_tool_type = ToolType::Ellipse;
						self.dispatch_response(Response::SetActiveTool {
							tool_name: ToolType::Ellipse.to_string(),
						});
					}
					Key::KeyX => {
						editor_state.tool_state.swap_colors();
					}
					_ => (),
				}
			}
		}

		let (responses, operations) = editor_state
			.tool_state
			.tool_data
			.active_tool()?
			.handle_input(event, &editor_state.document, &editor_state.tool_state.document_tool_data);

		self.dispatch_operations(&mut editor_state.document, operations);
		// TODO - Dispatch Responses

		Ok(())
	}

	fn dispatch_operations<I: IntoIterator<Item = Operation>>(&self, document: &mut Document, operations: I) {
		for operation in operations {
			if let Err(error) = self.dispatch_operation(document, operation) {
				log::error!("{}", error);
			}
		}
	}

	fn dispatch_operation(&self, document: &mut Document, operation: Operation) -> Result<(), EditorError> {
		document.handle_operation(operation, |svg: String| self.dispatch_response(Response::UpdateCanvas { document: svg }))?;
		Ok(())
	}

	pub fn dispatch_responses<I: IntoIterator<Item = Response>>(&self, responses: I) {
		for response in responses {
			self.dispatch_response(response);
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
