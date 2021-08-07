use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use glam::{DAffine2, Vec2Swizzles};
use graphene::Operation;

#[derive(Clone, Debug, Default)]
pub struct Resize {
	pub drag_start: ViewportPosition,
	pub path: Option<Vec<LayerId>>,
}
impl Resize {
	pub fn calculate_transform(&self, center: Key, lock_ratio: Key, ipp: &InputPreprocessor) -> Option<Message> {
		let mut start = self.drag_start.as_f64();
		let stop = ipp.mouse.position.as_f64();

		let mut size = stop - start;
		if ipp.keyboard.get(lock_ratio as usize) {
			size = size.abs().max(size.abs().yx()) * size.signum();
		}
		if ipp.keyboard.get(center as usize) {
			start -= size;
			size *= 2.;
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
