use crate::events::Event;
use crate::events::MouseKeys;
use crate::tools::Tool;
use document_core::Operation;

#[derive(Default)]
pub struct Select(Fsm);

impl Tool for Select {
	fn handle_input(&mut self, event: Event) -> Option<Operation> {
		match event {
			Event::MouseDown(state) => {
				if !(state.mouse_keys & MouseKeys::LEFT).is_empty() {
					self.0 = Fsm::LmbDown;
				}
			}
			Event::MouseUp(state) => {
				if self.0 == Fsm::LmbDown && state.mouse_keys == MouseKeys::LEFT {
					self.0 = Fsm::SelectedObject;
				}
			}
			_ => {}
		}

		None
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fsm {
	Ready,
	LmbDown,
	SelectedObject,
}

impl Default for Fsm {
	fn default() -> Self {
		Fsm::Ready
	}
}
