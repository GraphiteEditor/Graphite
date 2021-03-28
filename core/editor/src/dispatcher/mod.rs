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
		match event {
			Event::SelectTool(tool_type) => {
				state.tools.active_tool = tool_type;
				Ok(())
			}
			Event::SelectPrimaryColor(color) => {
				state.tools.primary_color = color;
				Ok(())
			}
			Event::SelectSecondaryColor(color) => {
				state.tools.secondary_color = color;
				Ok(())
			}
			Event::SwapColors => {
				std::mem::swap(&mut state.tools.primary_color, &mut state.tools.secondary_color);
				Ok(())
			}
			Event::ResetColors => {
				state.tools.primary_color = Color::BLACK;
				state.tools.secondary_color = Color::WHITE;
				Ok(())
			}
			Event::MouseDown(mouse_state) => {
				state.tools.mouse_state = mouse_state;
				// the state has changed so we add a trace point
				state.tools.record_trace_point();

				// self.emit_response(Response::UpdateCanvas { document: state.document.render() });
				Ok(())
			}
			Event::MouseUp(mouse_state) => {
				state.tools.mouse_state = mouse_state;
				// the state has changed so we add a trace point
				state.tools.record_trace_point();

				state.document.svg.push(SvgElement::Circle(Circle {
					center: Point {
						x: mouse_state.position.x as f64,
						y: mouse_state.position.y as f64,
					},
					radius: 10.0,
				}));

				self.emit_response(Response::UpdateCanvas { document: state.document.render() });
				Ok(())
			}
			Event::MouseMovement(pos) => {
				state.tools.mouse_state.position = pos;
				state.tools.record_trace_point();
				Ok(())
			}
			Event::ModifierKeyDown(mod_keys) => {
				state.tools.mod_keys = mod_keys;
				// the state has changed so we add a trace point
				state.tools.record_trace_point();
				Ok(())
			}
			Event::ModifierKeyUp(mod_keys) => {
				state.tools.mod_keys = mod_keys;
				// the state has changed so we add a trace point
				state.tools.record_trace_point();
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
