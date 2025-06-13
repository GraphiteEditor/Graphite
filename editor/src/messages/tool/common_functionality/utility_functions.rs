use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_text;
use crate::messages::tool::tool_messages::path_tool::PathOverlayMode;
use bezier_rs::Bezier;
use glam::DVec2;
use graphene_core::renderer::Quad;
use graphene_core::text::{FontCache, load_face};
use graphene_std::vector::{ManipulatorPointId, PointId, SegmentId, VectorData};

/// Determines if a path should be extended. Goal in viewport space. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(
	document: &DocumentMessageHandler,
	goal: DVec2,
	tolerance: f64,
	layers: impl Iterator<Item = LayerNodeIdentifier>,
	preferences: &PreferencesMessageHandler,
) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	closest_point(document, goal, tolerance, layers, |_| false, preferences)
}

/// Determine the closest point to the goal point under max_distance.
/// Additionally exclude checking closeness to the point which given to exclude() returns true.
pub fn closest_point<T>(
	document: &DocumentMessageHandler,
	goal: DVec2,
	max_distance: f64,
	layers: impl Iterator<Item = LayerNodeIdentifier>,
	exclude: T,
	preferences: &PreferencesMessageHandler,
) -> Option<(LayerNodeIdentifier, PointId, DVec2)>
where
	T: Fn(PointId) -> bool,
{
	let mut best = None;
	let mut best_distance_squared = max_distance * max_distance;
	for layer in layers {
		let viewspace = document.metadata().transform_to_viewport(layer);
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
		for id in vector_data.extendable_points(preferences.vector_meshes) {
			if exclude(id) {
				continue;
			}
			let Some(point) = vector_data.point_domain.position_from_id(id) else { continue };

			let distance_squared = viewspace.transform_point2(point).distance_squared(goal);

			if distance_squared < best_distance_squared {
				best = Some((layer, id, point));
				best_distance_squared = distance_squared;
			}
		}
	}

	best
}

/// Calculates the bounding box of the layer's text, based on the settings for max width and height specified in the typesetting config.
pub fn text_bounding_box(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, font_cache: &FontCache) -> Quad {
	let Some((text, font, typesetting)) = get_text(layer, &document.network_interface) else {
		return Quad::from_box([DVec2::ZERO, DVec2::ZERO]);
	};

	let buzz_face = font_cache.get(font).map(|data| load_face(data));
	let far = graphene_core::text::bounding_box(text, buzz_face.as_ref(), typesetting, false);

	Quad::from_box([DVec2::ZERO, far])
}

pub fn calculate_segment_angle(anchor: PointId, segment: SegmentId, vector_data: &VectorData, prefer_handle_direction: bool) -> Option<f64> {
	let is_start = |point: PointId, segment: SegmentId| vector_data.segment_start_from_id(segment) == Some(point);
	let anchor_position = vector_data.point_domain.position_from_id(anchor)?;
	let end_handle = ManipulatorPointId::EndHandle(segment).get_position(vector_data);
	let start_handle = ManipulatorPointId::PrimaryHandle(segment).get_position(vector_data);

	let start_point = if is_start(anchor, segment) {
		vector_data.segment_end_from_id(segment).and_then(|id| vector_data.point_domain.position_from_id(id))
	} else {
		vector_data.segment_start_from_id(segment).and_then(|id| vector_data.point_domain.position_from_id(id))
	};

	let required_handle = if is_start(anchor, segment) {
		start_handle
			.filter(|&handle| prefer_handle_direction && handle != anchor_position)
			.or(end_handle.filter(|&handle| Some(handle) != start_point))
			.or(start_point)
	} else {
		end_handle
			.filter(|&handle| prefer_handle_direction && handle != anchor_position)
			.or(start_handle.filter(|&handle| Some(handle) != start_point))
			.or(start_point)
	};

	required_handle.map(|handle| -(handle - anchor_position).angle_to(DVec2::X))
}

/// Check whether a point is visible in the current overlay mode.
pub fn is_visible_point(
	manipulator_point_id: ManipulatorPointId,
	vector_data: &VectorData,
	path_overlay_mode: PathOverlayMode,
	frontier_handles_info: Option<HashMap<SegmentId, Vec<PointId>>>,
	selected_segments: Vec<SegmentId>,
	selected_points: &HashSet<ManipulatorPointId>,
) -> bool {
	match manipulator_point_id {
		ManipulatorPointId::Anchor(_) => true,
		ManipulatorPointId::EndHandle(segment_id) | ManipulatorPointId::PrimaryHandle(segment_id) => {
			match (path_overlay_mode, selected_points.len() == 1) {
				(PathOverlayMode::AllHandles, _) => true,
				(PathOverlayMode::SelectedPointHandles, _) | (PathOverlayMode::FrontierHandles, true) => {
					if selected_segments.contains(&segment_id) {
						return true;
					}

					// Either the segment is a part of selected segments or the opposite handle is a part of existing selection
					let Some(handle_pair) = manipulator_point_id.get_handle_pair(vector_data) else { return false };
					let other_handle = handle_pair[1].to_manipulator_point();

					// Return whether the list of selected points contain the other handle
					selected_points.contains(&other_handle)
				}
				(PathOverlayMode::FrontierHandles, false) => {
					let Some(anchor) = manipulator_point_id.get_anchor(vector_data) else {
						warn!("No anchor for selected handle");
						return false;
					};
					let Some(frontier_handles) = &frontier_handles_info else {
						warn!("No frontier handles info provided");
						return false;
					};

					frontier_handles.get(&segment_id).map(|anchors| anchors.contains(&anchor)).unwrap_or_default()
				}
			}
		}
	}
}

pub fn calculate_bezier_bbox(_bezier: Bezier) {
	// Get the bbox of bezier
}
