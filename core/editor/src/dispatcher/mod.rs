pub mod events;
use crate::tools::ToolState;
use crate::{Color, EditorError};
use events::{Event, Response};

pub type Callback = Box<dyn Fn(Response)>;
pub struct Dispatcher {
	callback: Callback,
}

impl Dispatcher {
	pub fn handle_event(&self, tool_state: &mut ToolState, event: Event) -> Result<(), EditorError> {
		match event {
			Event::SelectTool(tool_type) => {
				tool_state.active_tool = tool_type;
				Ok(())
			}
			Event::SelectPrimaryColor(color) => {
				tool_state.primary_color = color;
				Ok(())
			}
			Event::SelectSecondaryColor(color) => {
				tool_state.secondary_color = color;
				Ok(())
			}
			Event::SwapColors => {
				std::mem::swap(&mut tool_state.primary_color, &mut tool_state.secondary_color);
				Ok(())
			}
			Event::ResetColors => {
				tool_state.primary_color = Color::BLACK;
				tool_state.secondary_color = Color::WHITE;
				Ok(())
			}
			Event::MouseDown(mouse_state) => {
				tool_state.mouse_state = mouse_state;
				// the state has changed so we add a trace point
				tool_state.record_trace_point();

				self.emit_response(Response::UpdateCanvas);
				Ok(())
			}
			Event::MouseUp(mouse_state) => {
				tool_state.mouse_state = mouse_state;
				// the state has changed so we add a trace point
				tool_state.record_trace_point();

				self.emit_response(Response::UpdateCanvas);
				Ok(())
			}
			Event::MouseMovement(pos) => {
				tool_state.mouse_state.position = pos;
				tool_state.record_trace_point();
				Ok(())
			}
			Event::ModifierKeyDown(mod_keys) => {
				tool_state.mod_keys = mod_keys;
				// the state has changed so we add a trace point
				tool_state.record_trace_point();
				Ok(())
			}
			Event::ModifierKeyUp(mod_keys) => {
				tool_state.mod_keys = mod_keys;
				// the state has changed so we add a trace point
				tool_state.record_trace_point();
				Ok(())
			}
			Event::KeyPress(key) => todo!(),
		}
	}
	pub fn emit_response(&self, response: Response) {
		let func = &self.callback;
		func(response)
	}
	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher { callback }
	}
}
