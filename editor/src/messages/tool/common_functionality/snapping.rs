use super::shape_editor::ManipulatorPointInfo;
use crate::consts::{SNAP_AXIS_TOLERANCE, SNAP_POINT_TOLERANCE};
use crate::messages::prelude::*;
use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::layer_info::LegacyLayer;
use document_legacy::LayerId;
use glam::DVec2;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

/// Handles snapping and snap overlays
#[derive(Debug, Clone, Default)]
pub struct SnapManager {
	point_targets: Option<Vec<DVec2>>,
	bound_targets: Option<Vec<DVec2>>,
	snap_x: bool,
	snap_y: bool,
}

impl SnapManager {
	/// Computes the necessary translation to the layer to snap it (as well as updating necessary overlays)
	fn calculate_snap<R>(&mut self, targets: R, responses: &mut VecDeque<Message>) -> DVec2
	where
		R: Iterator<Item = DVec2> + Clone,
	{
		let empty = Vec::new();
		let snap_points = self.snap_x && self.snap_y;

		let axis = self.bound_targets.as_ref().unwrap_or(&empty);
		let points = if snap_points { self.point_targets.as_ref().unwrap_or(&empty) } else { &empty };

		let x_axis = if self.snap_x { axis } else { &empty }
			.iter()
			.flat_map(|&pos| targets.clone().map(move |goal| (pos, goal, (pos - goal).x)));
		let y_axis = if self.snap_y { axis } else { &empty }
			.iter()
			.flat_map(|&pos| targets.clone().map(move |goal| (pos, goal, (pos - goal).y)));
		let points = points.iter().flat_map(|&pos| targets.clone().map(move |goal| (pos, pos - goal, (pos - goal).length())));

		let min_x = x_axis.clone().min_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("Could not compare position."));
		let min_y = y_axis.clone().min_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("Could not compare position."));
		let min_points = points.clone().min_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("Could not compare position."));

		// Snap to a point if possible
		let (clamped_closest_distance, _snapped_to_point) = if let Some(min_points) = min_points.filter(|&(_, _, dist)| dist <= SNAP_POINT_TOLERANCE) {
			(min_points.1, true)
		} else {
			// Do not move if over snap tolerance
			let closest_distance = DVec2::new(min_x.unwrap_or_default().2, min_y.unwrap_or_default().2);
			(
				DVec2::new(
					if closest_distance.x.abs() > SNAP_AXIS_TOLERANCE { 0. } else { closest_distance.x },
					if closest_distance.y.abs() > SNAP_AXIS_TOLERANCE { 0. } else { closest_distance.y },
				),
				false,
			)
		};
		responses.add(OverlaysMessage::Render);

		clamped_closest_distance
	}

	/// Gets a list of snap targets for the X and Y axes (if specified) in Viewport coords for the target layers (usually all layers or all non-selected layers.)
	/// This should be called at the start of a drag.
	pub fn start_snap(
		&mut self,
		document_message_handler: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		bounding_boxes: impl Iterator<Item = [DVec2; 2]>,
		snap_x: bool,
		snap_y: bool,
	) {
		let snapping_enabled = document_message_handler.snapping_state.snapping_enabled;
		let bounding_box_snapping = document_message_handler.snapping_state.bounding_box_snapping;
		if snapping_enabled && bounding_box_snapping {
			self.snap_x = snap_x;
			self.snap_y = snap_y;

			// Could be made into sorted Vec or a HashSet for more performant lookups.
			self.bound_targets = Some(
				bounding_boxes
					.flat_map(expand_bounds)
					.filter(|&pos| pos.x >= 0. && pos.y >= 0. && pos.x < input.viewport_bounds.size().x && pos.y <= input.viewport_bounds.size().y)
					.collect(),
			);
			self.point_targets = None;
		}
	}

	/// Add arbitrary snapping points
	///
	/// This should be called after start_snap
	pub fn add_snap_points(&mut self, document_message_handler: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, snap_points: impl Iterator<Item = DVec2>) {
		let snapping_enabled = document_message_handler.snapping_state.snapping_enabled;
		let node_snapping = document_message_handler.snapping_state.node_snapping;
		if snapping_enabled && node_snapping {
			let snap_points = snap_points.filter(|&pos| pos.x >= 0. && pos.y >= 0. && pos.x < input.viewport_bounds.size().x && pos.y <= input.viewport_bounds.size().y);
			if let Some(targets) = &mut self.point_targets {
				targets.extend(snap_points);
			} else {
				self.point_targets = Some(snap_points.collect());
			}
		}
	}

	/// Add the [ManipulatorGroup]s (optionally including handles) of the specified shape layer to the snapping points
	///
	/// This should be called after start_snap
	pub fn add_snap_path(
		&mut self,
		document_message_handler: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		layer: &LegacyLayer,
		path: &[LayerId],
		include_handles: bool,
		ignore_points: &[ManipulatorPointInfo],
	) {
		let Some(vector_data) = &layer.as_vector_data() else { return };

		if !document_message_handler.snapping_state.node_snapping {
			return;
		};

		let transform = document_message_handler.document_legacy.multiply_transforms(path).unwrap();
		let snap_points = vector_data
			.manipulator_groups()
			.flat_map(|group| {
				if include_handles {
					[
						Some((ManipulatorPointId::new(group.id, SelectedType::Anchor), group.anchor)),
						group.in_handle.map(|pos| (ManipulatorPointId::new(group.id, SelectedType::InHandle), pos)),
						group.out_handle.map(|pos| (ManipulatorPointId::new(group.id, SelectedType::OutHandle), pos)),
					]
				} else {
					[Some((ManipulatorPointId::new(group.id, SelectedType::Anchor), group.anchor)), None, None]
				}
			})
			.flatten()
			.filter(|&(point_id, _)| {
				!ignore_points.contains(&ManipulatorPointInfo {
					layer: LayerNodeIdentifier::from_path(path, document_message_handler.network()),
					point_id,
				})
			})
			.map(|(_, pos)| transform.transform_point2(pos));
		self.add_snap_points(document_message_handler, input, snap_points);
	}

	/// Adds all of the shape handles in the document, including bézier handles of the points specified
	pub fn add_all_document_handles(
		&mut self,
		document_message_handler: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		include_handles: &[&[LayerId]],
		exclude: &[&[LayerId]],
		ignore_points: &[ManipulatorPointInfo],
	) {
		for path in document_message_handler.all_layers() {
			if !exclude.contains(&path) {
				let layer = document_message_handler.document_legacy.layer(path).expect("Could not get layer for snapping");
				self.add_snap_path(document_message_handler, input, layer, path, include_handles.contains(&path), ignore_points);
			}
		}
	}

	/// Finds the closest snap from an array of layers to the specified snap targets in viewport coords.
	/// Returns 0 for each axis that there is no snap less than the snap tolerance.
	pub fn snap_layers(&mut self, responses: &mut VecDeque<Message>, document_message_handler: &DocumentMessageHandler, snap_anchors: Vec<DVec2>, mouse_delta: DVec2) -> DVec2 {
		if document_message_handler.snapping_state.snapping_enabled {
			self.calculate_snap(snap_anchors.iter().map(move |&snap| mouse_delta + snap), responses)
		} else {
			DVec2::ZERO
		}
	}

	/// Handles snapping of a viewport position, returning another viewport position.
	pub fn snap_position(&mut self, responses: &mut VecDeque<Message>, document_message_handler: &DocumentMessageHandler, position_viewport: DVec2) -> DVec2 {
		if document_message_handler.snapping_state.snapping_enabled {
			self.calculate_snap([position_viewport].into_iter(), responses) + position_viewport
		} else {
			position_viewport
		}
	}

	/// Removes snap target data and overlays. Call this when snapping is done.
	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.bound_targets = None;
		self.point_targets = None;
		responses.add(OverlaysMessage::Render);
	}
}

/// Converts a bounding box into a set of points for snapping
///
/// Puts a point in the middle of each edge (top, bottom, left, right)
pub fn expand_bounds([bound1, bound2]: [DVec2; 2]) -> [DVec2; 4] {
	[
		DVec2::new((bound1.x + bound2.x) / 2., bound1.y),
		DVec2::new((bound1.x + bound2.x) / 2., bound2.y),
		DVec2::new(bound1.x, (bound1.y + bound2.y) / 2.),
		DVec2::new(bound2.x, (bound1.y + bound2.y) / 2.),
	]
}
