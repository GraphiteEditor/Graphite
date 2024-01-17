use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use glam::{DAffine2, DVec2, Vec2Swizzles};

use super::snapping::{SnapCandidatePoint, SnapConstraint, SnapData};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	drag_start: ViewportPosition,
	pub layer: Option<LayerNodeIdentifier>,
	pub snap_manager: SnapManager,
}

impl Resize {
	/// Starts a resize, assigning the snap targets and snapping the starting position.
	pub fn start(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		let root_transform = document.metadata().document_to_viewport;
		let point = SnapCandidatePoint::handle(root_transform.inverse().transform_point2(input.mouse.position));
		let snapped = self.snap_manager.free_snap(&SnapData::new(document, input), &point, None, false);
		self.drag_start = snapped.snapped_point_document;
	}

	/// Calculate the drag start position in viewport space.
	pub fn viewport_drag_start(&self, document: &DocumentMessageHandler) -> DVec2 {
		let root_transform = document.metadata().document_to_viewport;
		root_transform.transform_point2(self.drag_start)
	}

	pub fn calculate_transform(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key, lock_ratio: Key, skip_rerender: bool) -> Option<Message> {
		let Some(layer) = self.layer else {
			return None;
		};
		if !document.network().nodes.contains_key(&layer.to_node()) {
			self.layer.take();
			return None;
		}

		let start = self.viewport_drag_start(document);
		let mouse = input.mouse.position;
		let to_viewport = document.metadata().document_to_viewport;
		let document_mouse = to_viewport.inverse().transform_point2(mouse);
		let mut points_viewport = [start, mouse];
		let ignore = if let Some(layer) = self.layer { vec![layer] } else { vec![] };
		let ratio = input.keyboard.get(lock_ratio as usize);
		let center = input.keyboard.get(center as usize);
		let snap_data = SnapData::ignore(document, input, &ignore);
		if ratio {
			let size = points_viewport[1] - points_viewport[0];
			let size = size.abs().max(size.abs().yx()) * size.signum();
			points_viewport[1] = points_viewport[0] + size;
			let end_document = to_viewport.inverse().transform_point2(points_viewport[1]);
			let constraint = SnapConstraint::Line {
				origin: self.drag_start,
				direction: end_document - self.drag_start,
			};
			if center {
				let snapped = self.snap_manager.constrained_snap(&snap_data, &SnapCandidatePoint::handle(end_document), constraint, None);
				let far = SnapCandidatePoint::handle(2. * self.drag_start - end_document);
				let snapped_far = self.snap_manager.constrained_snap(&snap_data, &far, constraint, None);
				let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
				points_viewport[0] = to_viewport.transform_point2(best.snapped_point_document);
				points_viewport[1] = to_viewport.transform_point2(self.drag_start * 2. - best.snapped_point_document);
				self.snap_manager.update_indicator(best);
			} else {
				let snapped = self.snap_manager.constrained_snap(&snap_data, &SnapCandidatePoint::handle(end_document), constraint, None);
				points_viewport[1] = to_viewport.transform_point2(snapped.snapped_point_document);
				self.snap_manager.update_indicator(snapped);
			}
		} else if center {
			let snapped = self.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), None, false);
			let snapped_far = self.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(2. * self.drag_start - document_mouse), None, false);
			let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
			points_viewport[0] = to_viewport.transform_point2(best.snapped_point_document);
			points_viewport[1] = to_viewport.transform_point2(self.drag_start * 2. - best.snapped_point_document);
			self.snap_manager.update_indicator(best);
		} else {
			let snapped = self.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), None, false);
			points_viewport[1] = to_viewport.transform_point2(snapped.snapped_point_document);
			self.snap_manager.update_indicator(snapped);
		}

		Some(
			GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_scale_angle_translation(points_viewport[1] - points_viewport[0], 0., points_viewport[0]),
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
