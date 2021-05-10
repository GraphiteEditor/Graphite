pub mod actions;
pub mod document_event_handler;
pub mod events;
pub mod global_event_handler;
pub mod input_manager;

use crate::{tools::ToolType, Color, Document, EditorError};
use document_core::Operation;
pub use events::{DocumentResponse, Event, Key, Response, ToolResponse};

pub use self::input_manager::InputPreprocessor;

pub use actions::Action;

pub type Callback = Box<dyn Fn(Response)>;

pub trait ActionHandler<T> {
	/// Return true if the Action is consumed.
	fn process_action<'a>(&mut self, data: T, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool;
	fn actions(&self) -> &[(&str, Action)];
}

pub struct Dispatcher {
	callback: Callback,
	input_preprocessor: InputPreprocessor,
}

impl Dispatcher {
	pub fn handle_event(&self, event: &Event) -> Result<(), EditorError> {
		log::trace!("{:?}", event);

		/*match event {
				Event::SelectTool(tool_name) => {
					editor_state.tool_state.tool_data.active_tool_type = *tool_name;
					self.dispatch_response(ToolResponse::SetActiveTool { tool_name: tool_name.to_string() });
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
				Event::LmbDown(mouse_state) | Event::RmbDown(mouse_state) | Event::MmbDown(mouse_state) | Event::LmbUp(mouse_state) | Event::RmbUp(mouse_state) | Event::MmbUp(mouse_state) => {
					editor_state.tool_state.document_tool_data.mouse_state = *mouse_state;
				}
				Event::MouseMove(pos) => {
					editor_state.tool_state.document_tool_data.mouse_state.position = *pos;
				}
				Event::ToggleLayerVisibility(path) => {
					log::debug!("Toggling layer visibility not yet implemented in the Editor Library");
				}
				Event::KeyUp(_key) => (),
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
							self.dispatch_response(ToolResponse::SetActiveTool {
								tool_name: ToolType::Select.to_string(),
							});
						}
						Key::KeyL => {
							editor_state.tool_state.tool_data.active_tool_type = ToolType::Line;
							self.dispatch_response(ToolResponse::SetActiveTool {
								tool_name: ToolType::Line.to_string(),
							});
						}
						Key::KeyP => {
							editor_state.tool_state.tool_data.active_tool_type = ToolType::Pen;
							self.dispatch_response(ToolResponse::SetActiveTool { tool_name: ToolType::Pen.to_string() });
						}
						Key::KeyM => {
							editor_state.tool_state.tool_data.active_tool_type = ToolType::Rectangle;
							self.dispatch_response(ToolResponse::SetActiveTool {
								tool_name: ToolType::Rectangle.to_string(),
							});
						}
						Key::KeyY => {
							editor_state.tool_state.tool_data.active_tool_type = ToolType::Shape;
							self.dispatch_response(ToolResponse::SetActiveTool {
								tool_name: ToolType::Shape.to_string(),
							});
						}
						Key::KeyE => {
							editor_state.tool_state.tool_data.active_tool_type = ToolType::Ellipse;
							self.dispatch_response(ToolResponse::SetActiveTool {
								tool_name: ToolType::Ellipse.to_string(),
							});
						}
						Key::KeyX => {
							editor_state.tool_state.swap_colors();
						}
						_ => (),
					}
				}
				_ => todo!("Implement layer handling"),
			}

		let (mut tool_responses, operations) = editor_state
			.tool_state
			.tool_data
			.active_tool()?
			.handle_input(event, &editor_state.document, &editor_state.tool_state.document_tool_data);

		let mut document_responses = self.dispatch_operations(&mut editor_state.document, operations);
		//let changes = document_responses.drain_filter(|x| x == DocumentResponse::DocumentChanged);
		let mut canvas_dirty = false;
		let mut i = 0;
		while i < document_responses.len() {
			if matches!(document_responses[i], DocumentResponse::DocumentChanged) {
				canvas_dirty = true;
				document_responses.remove(i);
			} else {
				i += 1;
			}
		}
		if canvas_dirty {
			tool_responses.push(ToolResponse::UpdateCanvas {
				document: editor_state.document.render_root(),
			})
		}
		self.dispatch_responses(tool_responses);
		self.dispatch_responses(document_responses);
		*/

		Ok(())
	}

	pub fn dispatch_responses<T: Into<Response>, I: IntoIterator<Item = T>>(&self, responses: I) {
		for response in responses {
			self.dispatch_response(response);
		}
	}

	pub fn dispatch_response<T: Into<Response>>(&self, response: T) {
		let func = &self.callback;
		let response: Response = response.into();
		log::trace!("Sending {} Response", response);
		func(response)
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			callback,
			input_preprocessor: InputPreprocessor::default(),
		}
	}
}
