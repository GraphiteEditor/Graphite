use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use glam::{DAffine2, DVec2, Vec2Swizzles};

use super::snapping::{SnapCandidatePoint, SnapData};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	drag_start: ViewportPosition,
	pub layer: Option<LayerNodeIdentifier>,
	pub snap_manager: SnapManager,
}

impl Resize {
	/// Starts a resize, assigning the snap targets and snapping the starting position.
	pub fn start(&mut self, responses: &mut VecDeque<Message>, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		let root_transform = document.metadata().document_to_viewport;
		let point = SnapCandidatePoint::new_handle(root_transform.inverse().transform_point2(input.mouse.position));
		let snapped = self.snap_manager.free_snap(SnapData::new(document, input), &point, None, false);
		self.drag_start = snapped.snapped_point_document;
	}

	/// Recalculates snap targets without snapping the starting position.
	pub fn recalculate_snaps(&mut self, _document: &DocumentMessageHandler, _input: &InputPreprocessorMessageHandler) {
		//	self.snap_manager.start_snap(document, input, document.bounding_boxes(), true, true);
		//self.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);
	}

	/// Calculate the drag start position in viewport space.
	pub fn viewport_drag_start(&self, document: &DocumentMessageHandler) -> DVec2 {
		let root_transform = document.metadata().document_to_viewport;
		root_transform.transform_point2(self.drag_start)
	}

	pub fn calculate_transform(
		&mut self,
		responses: &mut VecDeque<Message>,
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		center: Key,
		lock_ratio: Key,
		skip_rerender: bool,
	) -> Option<Message> {
		let Some(layer) = self.layer else {
			return None;
		};
		if !document.network().nodes.contains_key(&layer.to_node()) {
			self.layer.take();
			return None;
		}

		let mut start = self.viewport_drag_start(document);
		let point = SnapCandidatePoint::new_handle(document.metadata().document_to_viewport.inverse().transform_point2(ipp.mouse.position));
		let ignore = if let Some(x) = self.layer { vec![x] } else { vec![] };
		let snapped_point = self.snap_manager.free_snap(SnapData::ignore(document, ipp, &ignore), &point, None, false);
		let stop = document.metadata().document_to_viewport.transform_point2(snapped_point.snapped_point_document);
		self.snap_manager.update_indicator(snapped_point);

		let mut size = stop - start;
		if ipp.keyboard.get(lock_ratio as usize) {
			size = size.abs().max(size.abs().yx()) * size.signum();
		}
		if ipp.keyboard.get(center as usize) {
			start -= size;
			size *= 2.;
		}

		Some(
			GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_scale_angle_translation(size, 0., start),
				transform_in: TransformIn::Viewport,
				skip_rerender,
			}
			.into(),
		)
	}

	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.snap_manager.cleanup(responses);
		self.layer = None;
	}
}
