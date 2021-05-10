use super::{
	events::{Event, Key, MouseState},
	Action,
};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct KeyState {
	depressed: bool,
	// time of last press
	// mod keys held down while pressing
	// …
}

#[derive(Debug, Default)]
pub struct InputPreprocessor {
	mouse_keys: MouseState,
	keyboard: HashMap<Key, KeyState>,
	//key_translation: HashMap<Key, VirtualInputAction>,
	pub mouse_state: MouseState,
}

impl InputPreprocessor {
	pub fn handle_user_input(&mut self, event: Event) -> Vec<Action> {
		// clean user input and if possible reconstruct it
		// store the changes in the keyboard if it is a key event
		// translate the key events to VirtualKeyActions and return them
		// transform canvas coordinates to document coordinates
		// Last pressed key
		// respect text input mode
		vec![self.dummy_translation(event)]
	}

	fn dummy_translation(&mut self, event: Event) -> Action {
		match event {
			Event::SelectTool(tool_name) => Action::SelectTool(tool_name),
			Event::SelectPrimaryColor(color) => Action::SelectPrimaryColor(color),
			Event::SelectSecondaryColor(color) => Action::SelectSecondaryColor(color),
			Event::SwapColors => Action::SwapColors,
			Event::ResetColors => Action::ResetColors,
			Event::MouseMove(pos) => {
				self.mouse_state.position = pos;
				Action::MouseMove
			}
			Event::ToggleLayerVisibility(path) => {
				log::debug!("Toggling layer visibility not yet implemented in the Editor Library");
				Action::ToggleLayerVisibility(path)
			}
			Event::LmbDown(mouse_state) | Event::RmbDown(mouse_state) | Event::MmbDown(mouse_state) | Event::LmbUp(mouse_state) | Event::RmbUp(mouse_state) | Event::MmbUp(mouse_state) => {
				self.mouse_state = mouse_state;
				match event {
					Event::LmbDown(_) => Action::LmbDown,
					Event::LmbUp(_) => Action::LmbUp,
					Event::RmbDown(_) => Action::RmbDown,
					Event::RmbUp(_) => Action::RmbUp,
					Event::MmbDown(_) => Action::MmbDown,
					Event::MmbUp(_) => Action::MmbUp,
					_ => panic!(),
				}
			}
			_ => Action::Save,
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
	}
}
