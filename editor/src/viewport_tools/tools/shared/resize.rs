use crate::document::DocumentMessageHandler;
use crate::input::keyboard::Key;
use crate::input::mouse::ViewportPosition;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::viewport_tools::snapping::SnapHandler;

use graphene::Operation;

use glam::{DAffine2, DVec2, Vec2Swizzles};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	pub drag_start: ViewportPosition,
	pub path: Option<Vec<LayerId>>,
	snap_handler: SnapHandler,
}
impl Resize {
	/// Starts a resize, assigning the snap targets and snapping the starting position.
	pub fn start(&mut self, responses: &mut VecDeque<Message>, viewport_bounds: DVec2, document: &DocumentMessageHandler, mouse_position: DVec2) {
		self.snap_handler.start_snap(document, document.bounding_boxes(None, None), true, true);
		self.drag_start = self.snap_handler.snap_position(responses, viewport_bounds, document, mouse_position);
	}

	pub fn calculate_transform(
		&mut self,
		responses: &mut VecDeque<Message>,
		viewport_bounds: DVec2,
		document: &DocumentMessageHandler,
		center: Key,
		lock_ratio: Key,
		ipp: &InputPreprocessorMessageHandler,
	) -> Option<Message> {
		if let Some(path) = &self.path {
			let mut start = self.drag_start;

			let stop = self.snap_handler.snap_position(responses, viewport_bounds, document, ipp.mouse.position);

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

	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.snap_handler.cleanup(responses);
		self.path = None;
	}
}
