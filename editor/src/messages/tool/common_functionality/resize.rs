use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
use crate::messages::{input_mapper::utility_types::input_keyboard::Key, portfolio::document::graph_operation::utility_types::TransformIn};
use glam::{DAffine2, DVec2, Vec2Swizzles};

#[derive(Clone, Debug, Default)]
pub struct Resize {
	/// Stored as a document position so the start doesn't move if the canvas is panned.
	drag_start: DVec2,
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

		if !document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node()) {
			self.layer.take();
			return None;
		}
		Some(self.calculate_points_ignore_layer(document, input, center, lock_ratio))
	}

	/// Compute the drag start and end based on the current mouse position. Ignores the state of the layer.
	/// If you want to only draw whilst a layer exists, use [`Resize::calculate_points`].
	pub fn calculate_points_ignore_layer(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key, lock_ratio: Key) -> [DVec2; 2] {
		let start = self.drag_start;
		let mouse = input.mouse.position;
		let document_to_viewport = document.navigation_handler.calculate_offset_transform(input.viewport_bounds.center(), &document.document_ptz);
		let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
		let mut document_points = [start, document_mouse];

		let ignore = if let Some(layer) = self.layer { vec![layer] } else { vec![] };
		let ratio = input.keyboard.get(lock_ratio as usize);
		let center = input.keyboard.get(center as usize);

		let snap_data = SnapData::ignore(document, input, &ignore);
		let config = SnapTypeConfiguration::default();
		if ratio {
			let size = document_points[1] - document_points[0];
			let size = size.abs().max(size.abs().yx()) * size.signum();
			document_points[1] = document_points[0] + size;
			let end = document_points[1];
			let constraint = SnapConstraint::Line {
				origin: self.drag_start,
				direction: end - self.drag_start,
			};
			if center {
				let snapped = self.snap_manager.constrained_snap(&snap_data, &SnapCandidatePoint::handle(end), constraint, config);
				let far = SnapCandidatePoint::handle(2. * self.drag_start - end);
				let snapped_far = self.snap_manager.constrained_snap(&snap_data, &far, constraint, config);
				let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
				document_points[0] = best.snapped_point_document;
				document_points[1] = self.drag_start * 2. - best.snapped_point_document;
				self.snap_manager.update_indicator(best);
			} else {
				let snapped = self.snap_manager.constrained_snap(&snap_data, &SnapCandidatePoint::handle(end), constraint, config);
				document_points[1] = snapped.snapped_point_document;
				self.snap_manager.update_indicator(snapped);
			}
		} else if center {
			let snapped = self.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), config);
			let opposite = 2. * self.drag_start - document_mouse;
			let snapped_far = self.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(opposite), config);
			let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
			document_points[0] = best.snapped_point_document;
			document_points[1] = self.drag_start * 2. - best.snapped_point_document;
			self.snap_manager.update_indicator(best);
		} else {
			let snapped = self.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), config);
			document_points[1] = snapped.snapped_point_document;
			self.snap_manager.update_indicator(snapped);
		}

		document_points
	}

	pub fn calculate_transform(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, center: Key, lock_ratio: Key, skip_rerender: bool) -> Option<Message> {
		let viewport_points = self.calculate_points(document, input, center, lock_ratio).map(|points| {
			let document_to_viewport = document.metadata().document_to_viewport;
			[document_to_viewport.transform_point2(points[0]), document_to_viewport.transform_point2(points[1])]
		})?;

		Some(
			GraphOperationMessage::TransformSet {
				layer: self.layer?,
				transform: DAffine2::from_scale_angle_translation(viewport_points[1] - viewport_points[0], 0., viewport_points[0]),
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
