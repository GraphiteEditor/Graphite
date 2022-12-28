use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapManager;

use document_legacy::layers::text_layer::FontCache;
use document_legacy::LayerId;
use document_legacy::Operation;

use glam::{DAffine2, DVec2, Vec2Swizzles};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	drag_start: ViewportPosition,
	pub path: Option<Vec<LayerId>>,
	snap_manager: SnapManager,
}

impl Resize {
	/// Starts a resize, assigning the snap targets and snapping the starting position.
	pub fn start(&mut self, responses: &mut VecDeque<Message>, document: &DocumentMessageHandler, mouse_position: DVec2, font_cache: &FontCache) {
		self.snap_manager.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
		self.snap_manager.add_all_document_handles(document, &[], &[], &[]);
		let root_transform = document.document_legacy.root.transform;
		self.drag_start = root_transform.inverse().transform_point2(self.snap_manager.snap_position(responses, document, mouse_position));
	}

	/// Calculate the drag start position in viewport space.
	pub fn viewport_drag_start(&self, document: &DocumentMessageHandler) -> DVec2 {
		let root_transform = document.document_legacy.root.transform;
		root_transform.transform_point2(self.drag_start)
	}

	pub fn calculate_transform(
		&mut self,
		responses: &mut VecDeque<Message>,
		document: &DocumentMessageHandler,
		center: Key,
		lock_ratio: Key,
		ipp: &InputPreprocessorMessageHandler,
	) -> Option<Message> {
		if let Some(path) = &self.path {
			let mut start = self.viewport_drag_start(document);

			let stop = self.snap_manager.snap_position(responses, document, ipp.mouse.position);

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
		self.snap_manager.cleanup(responses);
		self.path = None;
	}
}
