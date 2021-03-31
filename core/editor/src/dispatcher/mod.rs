pub mod events;
use crate::{Color, EditorError, EditorState};
use document_core::{Circle, Point, SvgElement};
use events::{Event, Response};

pub type Callback = Box<dyn Fn(Response)>;
pub struct Dispatcher {
	callback: Callback,
}

impl Dispatcher {
	pub fn handle_event(&self, state: &mut EditorState, event: Event) -> Result<(), EditorError> {
		log::trace!("{:?}", event);
		match event {
			Event::SelectTool(tool_type) => {
				state.tool_state.active_tool_type = tool_type;
				Ok(())
			}
			Event::SelectPrimaryColor(color) => {
				state.tool_state.primary_color = color;
				Ok(())
			}
			Event::SelectSecondaryColor(color) => {
				state.tool_state.secondary_color = color;
				Ok(())
			}
			Event::SwapColors => {
				std::mem::swap(&mut state.tool_state.primary_color, &mut state.tool_state.secondary_color);
				Ok(())
			}
			Event::ResetColors => {
				state.tool_state.primary_color = Color::BLACK;
				state.tool_state.secondary_color = Color::WHITE;
				Ok(())
			}
			Event::MouseDown(mouse_state) => {
				state.tool_state.mouse_state = mouse_state;
				// The state has changed so we add a trace point.
				state.tool_state.record_trace_point();

				state.tool_state.active_tool().handle_input(event);
				Ok(())
			}
			Event::MouseUp(mouse_state) => {
				state.tool_state.mouse_state = mouse_state;
				// The state has changed so we add a trace point.
				state.tool_state.record_trace_point();

				state.document.svg.push(SvgElement::Circle(Circle {
					center: Point {
						x: mouse_state.position.x as f64,
						y: mouse_state.position.y as f64,
					},
					radius: 10.0,
				}));

				self.emit_response(Response::UpdateCanvas { document: state.document.render() });
				state.tool_state.active_tool().handle_input(event);
				Ok(())
			}
			Event::MouseMovement(pos) => {
				state.tool_state.mouse_state.position = pos;
				// The state has changed so we add a trace point.
				state.tool_state.record_trace_point();
				state.tool_state.active_tool().handle_input(event);
				Ok(())
			}
			Event::ModifierKeyDown(mod_keys) => {
				state.tool_state.mod_keys = mod_keys;
				// The state has changed so we add a trace point.
				state.tool_state.record_trace_point();
				state.tool_state.active_tool().handle_input(event);
				Ok(())
			}
			Event::ModifierKeyUp(mod_keys) => {
				state.tool_state.mod_keys = mod_keys;
				// The state has changed so we add a trace point.
				state.tool_state.record_trace_point();
				state.tool_state.active_tool().handle_input(event);
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
