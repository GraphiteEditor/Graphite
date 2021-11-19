use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use crate::tool::DocumentMessageHandler;
use glam::{DAffine2, DVec2, Vec2Swizzles};
use graphene::Operation;

#[derive(Clone, Debug, Default)]
pub struct Resize {
	pub drag_start: ViewportPosition,
	pub path: Option<Vec<LayerId>>,
}
impl Resize {
	/// Starts a resize, assigning the snap targets and snapping the starting position.
	pub fn start(&mut self, document: &mut DocumentMessageHandler, mouse_position: DVec2) {
		let layers = document.all_layers_sorted();
		document.snapping_handler.start_snap(&document.graphene_document, layers, &[]);
		self.drag_start = document.snapping_handler.snap_position(mouse_position);
	}

	pub fn calculate_transform(&self, document: &DocumentMessageHandler, center: Key, lock_ratio: Key, ipp: &InputPreprocessor) -> Option<Message> {
		if let Some(path) = &self.path {
			let mut start = self.drag_start;

			let stop = document.snapping_handler.snap_position(ipp.mouse.position);

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
