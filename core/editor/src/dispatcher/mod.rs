pub mod actions;
pub mod document_event_handler;
pub mod events;
pub mod global_event_handler;
pub mod input_manager;

use crate::EditorError;
use document_core::Operation;
pub use events::{DocumentResponse, Event, Key, Response, ToolResponse};

use self::global_event_handler::GlobalEventHandler;
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
	global_event_handler: GlobalEventHandler,
	operations: Vec<Operation>,
	responses: Vec<Response>,
}

impl Dispatcher {
	pub fn handle_event(&mut self, event: Event) -> Result<(), EditorError> {
		log::trace!("{:?}", event);

		self.operations.clear();
		self.responses.clear();
		let actions = self.input_preprocessor.handle_user_input(event);
		for action in actions {
			self.handle_action(action);
		}
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
		*/

		Ok(())
	}

	fn handle_action(&mut self, action: Action) {
		let consumed = self
			.global_event_handler
			.process_action((), &self.input_preprocessor, &action, &mut self.responses, &mut self.operations);

		debug_assert!(self.operations.is_empty());

		self.dispatch_responses();

		if !consumed {
			log::warn!("Unhandled action {:?}", action);
		}
	}

	pub fn dispatch_responses(&mut self) {
		for response in self.responses.drain(..) {
			Self::dispatch_response(response, &self.callback);
		}
	}

	pub fn dispatch_response<T: Into<Response>>(response: T, callback: &Callback) {
		let response: Response = response.into();
		log::trace!("Sending {} Response", response);
		callback(response)
	}

	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher {
			callback,
			input_preprocessor: InputPreprocessor::default(),
			global_event_handler: GlobalEventHandler::default(),
			operations: Vec::new(),
			responses: Vec::new(),
		}
	}
}
