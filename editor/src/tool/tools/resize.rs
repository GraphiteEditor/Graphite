use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use crate::tool::snapping;
use crate::tool::DocumentMessageHandler;
use glam::{DAffine2, DVec2, Vec2Swizzles};
use graphene::Operation;

#[derive(Clone, Debug, Default)]
pub struct Resize {
	pub drag_start: ViewportPosition,
	pub path: Option<Vec<LayerId>>,
	snap_targets: Option<[Vec<f64>; 2]>,
}
impl Resize {
	/// Starts of a resize, assigning the snap targets and snapping and assigning the starting position.
	pub fn start(&mut self, document: &DocumentMessageHandler, mouse_position: DVec2) {
		let snap_targets = snapping::get_snap_targets(document, document.all_layers_sorted(), &[]);

		let snapped_position = snapping::snap_position(&snap_targets, mouse_position);

		self.drag_start = snapped_position;

		self.snap_targets = Some(snap_targets);
	}

	pub fn calculate_transform(&self, center: Key, lock_ratio: Key, ipp: &InputPreprocessor) -> Option<Message> {
		if let (Some(path), Some(snap_targets)) = (&self.path, &self.snap_targets) {
			let mut start = self.drag_start;
			let stop = snapping::snap_position(snap_targets, ipp.mouse.position);

			let mut size = stop - start;
			if ipp.keyboard.get(lock_ratio as usize) {
				size = size.abs().max(size.abs().yx()) * size.signum();
			}
			if ipp.keyboard.get(center as usize) {
				start -= size;
				size *= 2.;
			}

			Some(
				Operation::SetLayerTransformInViewport {
					path: path.to_vec(),
					transform: DAffine2::from_scale_angle_translation(size, 0., start).to_cols_array(),
				}
				.into(),
			)
		} else {
			None
		}
	}
}
