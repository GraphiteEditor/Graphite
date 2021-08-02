use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use document_core::Operation;
use glam::{DAffine2, DVec2, Vec2Swizzles};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	pub drag_start: ViewportPosition,
	pub drag_current: ViewportPosition,
	pub constrain_to_square: bool,
	pub center_around_cursor: bool,
	pub path: Option<Vec<LayerId>>,
}

#[impl_message]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum ResizeMessage {
	PointerMove,
	Center,
	UnCenter,
	LockAspectRatio,
	UnlockAspectRatio,
}

impl<'a> MessageHandler<ResizeMessage, &InputPreprocessor> for Resize {
	fn process_action(&mut self, action: ResizeMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		self.drag_current = ipp.mouse.position;
		use ResizeMessage::*;
		match action {
			PointerMove => self.drag_current = ipp.mouse.position,
			LockAspectRatio => self.constrain_to_square = true,
			UnlockAspectRatio => self.constrain_to_square = false,
			Center => self.center_around_cursor = true,
			UnCenter => self.center_around_cursor = false,
		}
		if let Some(message) = self.calculate_transform() {
			responses.push_back(message);
		}
	}
	fn actions(&self) -> ActionList {
		vec![]
	}
}
impl Resize {
	fn calculate_transform(&self) -> Option<Message> {
		let mut start = self.drag_start.as_f64();
		let stop = self.drag_current.as_f64();

		let mut size = stop - start;
		if self.constrain_to_square {
			size = size.abs().max(size.abs().yx()) * size.signum();
		}
		if self.center_around_cursor {
			start -= size / 2.;
		}

		self.path.clone().map(|path| {
			Operation::SetLayerTransformInViewport {
				path,
				transform: DAffine2::from_scale_angle_translation(size, 0., start).to_cols_array(),
			}
			.into()
		})
	}
}
