use crate::events::Event;
use crate::events::MouseKeys;
use crate::tools::Tool;

pub struct Select(Fsm);

impl Tool for Select {
	fn handle_input(&mut self, event: Event) {
		match event {
			Event::MouseDown(state) => {
				if state.mouse_keys == MouseKeys::LEFT {
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
	}
}

impl Default for Select {
	fn default() -> Self {
		Self(Fsm::Ready)
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
