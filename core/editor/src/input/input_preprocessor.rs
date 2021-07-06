use super::keyboard::{Key, KeyStates};
use super::mouse::{MouseKeys, MouseState, ScrollDelta, ViewportPosition};
use crate::message_prelude::*;

#[doc(inline)]
pub use document_core::DocumentResponse;
use glam::DVec2;

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug)]
pub enum InputPreprocessorMessage {
	MouseDown(MouseState),
	MouseUp(MouseState),
	MouseMove(ViewportPosition),
	MouseScroll(ScrollDelta),
	KeyUp(Key),
	KeyDown(Key),
	ViewportResize(ViewportPosition),
}

#[derive(Debug, Default)]
pub struct InputPreprocessor {
	pub keyboard: KeyStates,
	pub mouse: MouseState,
	pub viewport_size: ViewportPosition,
}

enum KeyPosition {
	Pressed,
	Released,
}

impl MessageHandler<InputPreprocessorMessage, ()> for InputPreprocessor {
	fn process_action(&mut self, message: InputPreprocessorMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			InputPreprocessorMessage::MouseMove(pos) => {
				self.mouse.position = pos;
				responses.push_back(InputMapperMessage::PointerMove.into());
			}
			InputPreprocessorMessage::MouseScroll(delta) => {
				self.mouse.scroll_delta = delta;
				responses.push_back(InputMapperMessage::MouseScroll.into());
			}
			InputPreprocessorMessage::MouseDown(state) => {
				responses.push_back(self.translate_mouse_event(state, KeyPosition::Pressed));
			}
			InputPreprocessorMessage::MouseUp(state) => {
				responses.push_back(self.translate_mouse_event(state, KeyPosition::Released));
			}
			InputPreprocessorMessage::KeyDown(key) => {
				self.keyboard.set(key as usize);
				responses.push_back(InputMapperMessage::KeyDown(key).into())
			}
			InputPreprocessorMessage::KeyUp(key) => {
				self.keyboard.unset(key as usize);
				responses.push_back(InputMapperMessage::KeyUp(key).into());
			}
			InputPreprocessorMessage::ViewportResize(size) => {
				responses.push_back(
					document_core::Operation::TransformLayer {
						path: vec![],
						transform: glam::DAffine2::from_translation(DVec2::new((size.x as f64 - self.viewport_size.x as f64) / 2., (size.y as f64 - self.viewport_size.y as f64) / 2.)).to_cols_array(),
					}
					.into(),
				);
				self.viewport_size = size;
			}
		};
	}
	// clean user input and if possible reconstruct it
	// store the changes in the keyboard if it is a key event
	// transform canvas coordinates to document coordinates
	advertise_actions!();
}

impl InputPreprocessor {
	fn translate_mouse_event(&mut self, new_state: MouseState, position: KeyPosition) -> Message {
		// Calculate the difference between the two key states (binary xor)
		let diff = self.mouse.mouse_keys ^ new_state.mouse_keys;
		self.mouse = new_state;
		let key = match diff {
			MouseKeys::LEFT => Key::Lmb,
			MouseKeys::RIGHT => Key::Rmb,
			MouseKeys::MIDDLE => Key::Mmb,
			_ => {
				log::warn!("The number of buttons modified at the same time was not equal to 1. Modification: {:#010b}", diff);
				Key::UnknownKey
			}
		};
		match position {
			KeyPosition::Pressed => InputMapperMessage::KeyDown(key).into(),
			KeyPosition::Released => InputMapperMessage::KeyUp(key).into(),
		}
	}
}
