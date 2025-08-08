use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
use glam::{DAffine2, DVec2, Vec2Swizzles};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	/// Stored as a document position so the start doesn't move if the canvas is panned.
	pub drag_start: DVec2,
	pub layer: Option<LayerNodeIdentifier>,
	pub snap_manager: SnapManager,
}

impl Resize {
	/// Starts a resize, assigning the snap targets and snapping the starting position.
	pub fn start(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		let root_transform = document.metadata().document_to_viewport;
		let point = SnapCandidatePoint::handle(root_transform.inverse().transform_point2(input.mouse.position));
		let snapped = self.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
		self.drag_start = snapped.snapped_point_document;
	}

	/// Calculate the drag start position in viewport space.
	pub fn viewport_drag_start(&self, document: &DocumentMessageHandler) -> DVec2 {
		let root_transform = document.metadata().document_to_viewport;
		root_transform.transform_point2(self.drag_start)
	}

	/// Compute the drag start and end based on the current mouse position. If the layer doesn't exist, returns [`None`].
	/// If you want to draw even without a layer, use [`Resize::calculate_points_ignore_layer`].
	pub fn calculate_points(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key, lock_ratio: Key) -> Option<[DVec2; 2]> {
		let layer = self.layer?;

		if layer == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Resize layer cannot be ROOT_PARENT");
			return None;
		}

		if !document.network_interface.document_network().nodes.contains_key(&layer.to_node()) {
			self.layer.take();
			return None;
		}
		Some(self.calculate_points_ignore_layer(document, input, center, lock_ratio, false))
	}

	/// Compute the drag start and end based on the current mouse position. Ignores the state of the layer.
	/// If you want to only draw whilst a layer exists, use [`Resize::calculate_points`].
	pub fn calculate_points_ignore_layer(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key, lock_ratio: Key, in_document: bool) -> [DVec2; 2] {
		let ratio = input.keyboard.get(lock_ratio as usize);
		let center = input.keyboard.get(center as usize);

		// Use shared snapping logic with optional center and ratio constraints, considering if coordinates are in document space.
		self.compute_snapped_resize_points(document, input, center, ratio, in_document)
	}

	pub fn calculate_transform(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key, lock_ratio: Key, skip_rerender: bool) -> Option<Message> {
		let points_viewport = self.calculate_points(document, input, center, lock_ratio)?;
		Some(
			GraphOperationMessage::TransformSet {
				layer: self.layer?,
				transform: DAffine2::from_scale_angle_translation(points_viewport[1] - points_viewport[0], 0., points_viewport[0]),
				transform_in: TransformIn::Viewport,
				skip_rerender,
			}
			.into(),
		)
	}

	pub fn calculate_circle_points(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key) -> [DVec2; 2] {
		let center = input.keyboard.get(center as usize);

		// Use shared snapping logic with enforced aspect ratio and optional center snapping.
		self.compute_snapped_resize_points(document, input, center, true, false)
	}

	/// Calculates two points in viewport space from a drag, applying snapping, optional center mode, and aspect ratio locking.
	fn compute_snapped_resize_points(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: bool, lock_ratio: bool, in_document: bool) -> [DVec2; 2] {
		let start = self.viewport_drag_start(document);
		let mouse = input.mouse.position;
		let document_to_viewport = document.navigation_handler.calculate_offset_transform(input.viewport_bounds.center(), &document.document_ptz);
		let drag_start = self.drag_start;
		let mut points_viewport = [start, mouse];

		let ignore = if let Some(layer) = self.layer { vec![layer] } else { vec![] };
		let snap_data = &SnapData::ignore(document, input, &ignore);

		if lock_ratio {
			let viewport_size = points_viewport[1] - points_viewport[0];
			let raw_size = if in_document {
				document_to_viewport.inverse().transform_vector2(viewport_size)
			} else {
				viewport_size
			};

			let adjusted_size = raw_size.abs().max(raw_size.abs().yx()) * raw_size.signum();
			let size = if in_document { document_to_viewport.transform_vector2(adjusted_size) } else { adjusted_size };

			points_viewport[1] = points_viewport[0] + size;
			let end_document = document_to_viewport.inverse().transform_point2(points_viewport[1]);
			let constraint = SnapConstraint::Line {
				origin: drag_start,
				direction: end_document - drag_start,
			};

			if center {
				let snapped = self
					.snap_manager
					.constrained_snap(snap_data, &SnapCandidatePoint::handle(end_document), constraint, SnapTypeConfiguration::default());
				let far = SnapCandidatePoint::handle(2. * drag_start - end_document);
				let snapped_far = self.snap_manager.constrained_snap(snap_data, &far, constraint, SnapTypeConfiguration::default());
				let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };

				points_viewport[0] = document_to_viewport.transform_point2(best.snapped_point_document);
				points_viewport[1] = document_to_viewport.transform_point2(drag_start * 2. - best.snapped_point_document);
				self.snap_manager.update_indicator(best);
			} else {
				let snapped = self
					.snap_manager
					.constrained_snap(snap_data, &SnapCandidatePoint::handle(end_document), constraint, SnapTypeConfiguration::default());
				points_viewport[1] = document_to_viewport.transform_point2(snapped.snapped_point_document);
				self.snap_manager.update_indicator(snapped);
			}
		} else {
			let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
			if center {
				let snapped = self.snap_manager.free_snap(snap_data, &SnapCandidatePoint::handle(document_mouse), SnapTypeConfiguration::default());
				let opposite = 2. * drag_start - document_mouse;
				let snapped_far = self.snap_manager.free_snap(snap_data, &SnapCandidatePoint::handle(opposite), SnapTypeConfiguration::default());
				let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };

				points_viewport[0] = document_to_viewport.transform_point2(best.snapped_point_document);
				points_viewport[1] = document_to_viewport.transform_point2(drag_start * 2. - best.snapped_point_document);
				self.snap_manager.update_indicator(best);
			} else {
				let snapped = self.snap_manager.free_snap(snap_data, &SnapCandidatePoint::handle(document_mouse), SnapTypeConfiguration::default());
				points_viewport[1] = document_to_viewport.transform_point2(snapped.snapped_point_document);
				self.snap_manager.update_indicator(snapped);
			}
		}

		points_viewport
	}

	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.snap_manager.cleanup(responses);
		self.layer = None;
	}
}
