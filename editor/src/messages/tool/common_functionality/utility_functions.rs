use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_text;
use crate::messages::tool::tool_messages::path_tool::PathOverlayMode;
use glam::DVec2;
use graphene_core::renderer::Quad;
use graphene_core::text::{FontCache, load_face};
use graphene_std::vector::{HandleId, ManipulatorPointId, PointId, SegmentId, VectorData, VectorModificationType};

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

pub fn molded_control_points(start: DVec2, end: DVec2, t: f64, falloff: f64, new_b: DVec2, c1: DVec2, c2: DVec2) -> (DVec2, DVec2) {
	let v1 = (1. - t) * start + t * c1;
	let a = (1. - t) * c1 + t * c2;
	let v2 = (1. - t) * c2 + t * end;
	let e1 = (1. - t) * v1 + t * a;
	let e2 = (1. - t) * a + t * v2;
	let b = (1. - t) * e1 + t * e2;

	let d1 = e1 - b;
	let d2 = e2 - b;
	let ne1 = new_b + d1;
	let ne2 = new_b + d2;

	// Calculate new points A and C (C stays the same)
	let point_c_ratio = (1. - t).powi(3) / (t.powi(3) + (1. - t).powi(3));
	let ab_bc_ratio = ((t.powi(3) + (1. - t).powi(3) - 1.) / (t.powi(3) + (1. - t).powi(3))).abs();
	let c = point_c_ratio * start + (1. - point_c_ratio) * end;
	let new_a = new_b + (new_b - c) / ab_bc_ratio;

	// Derive the new control points c1, c2
	let (nc1, nc2) = derive_control_points(t, new_a, ne1, ne2, start, end);

	// Calculate the idealized curve
	if let Some((ideal_c1, ideal_c2)) = get_idealized_cubic_curve(start, new_b, end) {
		let d = (b - new_b).length();
		let interpolation_ratio = d.min(falloff) / falloff;
		let ic1 = (1. - interpolation_ratio) * nc1 + interpolation_ratio * ideal_c1;
		let ic2 = (1. - interpolation_ratio) * nc2 + interpolation_ratio * ideal_c2;
		(ic1, ic2)
	} else {
		(nc1, nc2)
	}
}

pub fn get_idealized_cubic_curve(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<(DVec2, DVec2)> {
	use std::f64::consts::{PI, TAU};

	let center = calculate_center(p1, p2, p3)?;

	let d1 = (p1 - p2).length();
	let d2 = (p2 - p3).length();
	let t = d1 / (d1 + d2);

	let start = p1;
	let end = p3;

	let [a, b, _c] = compute_abc_for_cubic_through_points(p1, p2, p3, t);

	let angle = ((end.y - start.y).atan2(end.x - start.x) - (b.y - start.y).atan2(b.x - start.x) + TAU) % TAU;
	let factor = if !(0.0..=PI).contains(&angle) { -1. } else { 1. };
	let bc = factor * (start - end).length() / 3.;
	let de1 = t * bc;
	let de2 = (1. - t) * bc;
	let tangent = [
		DVec2::new(b.x - 10. * (b.y - center.y), b.y + 10. * (b.x - center.x)),
		DVec2::new(b.x + 10. * (b.y - center.y), b.y - 10. * (b.x - center.x)),
	];

	let normalized_tangent = (tangent[1] - tangent[0]).try_normalize()?;

	let e1 = DVec2::new(b.x + de1 * normalized_tangent.x, b.y + de1 * normalized_tangent.y);
	let e2 = DVec2::new(b.x - de2 * normalized_tangent.x, b.y - de2 * normalized_tangent.y);

	// Deriving control points
	Some(derive_control_points(t, a, e1, e2, start, end))
}

fn derive_control_points(t: f64, a: DVec2, e1: DVec2, e2: DVec2, start: DVec2, end: DVec2) -> (DVec2, DVec2) {
	let v1 = (e1 - t * a) / (1. - t);
	let v2 = (e2 - (1. - t) * a) / t;
	let c1 = (v1 - (1. - t) * start) / t;
	let c2 = (v2 - t * end) / (1. - t);
	(c1, c2)
}

fn calculate_center(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<DVec2> {
	// Calculate midpoints of two sides
	let mid1 = (p1 + p2) / 2.;
	let mid2 = (p2 + p3) / 2.;

	// Calculate perpendicular bisectors
	let dir1 = p2 - p1;
	let dir2 = p3 - p2;
	let perp_dir1 = DVec2::new(-dir1.y, dir1.x);
	let perp_dir2 = DVec2::new(-dir2.y, dir2.x);

	// Create points along the perpendicular directions
	let mid1_plus = mid1 + perp_dir1;
	let mid2_plus = mid2 + perp_dir2;

	// Find intersection of the two perpendicular bisectors
	line_line_intersection(mid1, mid1_plus, mid2, mid2_plus)
}

fn line_line_intersection(a1: DVec2, a2: DVec2, b1: DVec2, b2: DVec2) -> Option<DVec2> {
	let (x1, y1) = (a1.x, a1.y);
	let (x2, y2) = (a2.x, a2.y);
	let (x3, y3) = (b1.x, b1.y);
	let (x4, y4) = (b2.x, b2.y);

	// Calculate numerator components
	let nx = (x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4);
	let ny = (x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4);

	// Calculate denominator
	let d = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);

	// Check for parallel lines (colinear points)
	if d.abs() < f64::EPSILON { None } else { Some(DVec2::new(nx / d, ny / d)) }
}

pub fn compute_abc_for_cubic_through_points(start_point: DVec2, point_on_curve: DVec2, end_point: DVec2, t: f64) -> [DVec2; 3] {
	let point_c_ratio = (1. - t).powi(3) / (t.powi(3) + (1. - t).powi(3));
	let c = point_c_ratio * start_point + (1. - point_c_ratio) * end_point;

	let ab_bc_ratio = ((t.powi(3) + (1. - t).powi(3) - 1.) / (t.powi(3) + (1. - t).powi(3))).abs();
	let a = point_on_curve + (point_on_curve - c) / ab_bc_ratio;
	[a, point_on_curve, c]
}

pub fn adjust_handle_colinearity(handle: HandleId, anchor_position: DVec2, target_control_point: DVec2, vector_data: &VectorData, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(other_handle) = vector_data.other_colinear_handle(handle) else { return };
	let Some(handle_position) = other_handle.to_manipulator_point().get_position(vector_data) else {
		return;
	};
	let Some(direction) = (anchor_position - target_control_point).try_normalize() else { return };

	let new_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(new_relative_position);

	responses.add(GraphOperationMessage::Vector { layer, modification_type });
}

pub fn disable_g1_continuity(handle: HandleId, vector_data: &VectorData, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(other_handle) = vector_data.other_colinear_handle(handle) else { return };
	let handles = [handle, other_handle];
	let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };

	responses.add(GraphOperationMessage::Vector { layer, modification_type });
}

pub fn restore_previous_handle_position(
	handle: HandleId,
	original_c: DVec2,
	anchor_position: DVec2,
	vector_data: &VectorData,
	layer: LayerNodeIdentifier,
	responses: &mut VecDeque<Message>,
) -> Option<HandleId> {
	let other_handle = vector_data.other_colinear_handle(handle)?;
	let handle_position = other_handle.to_manipulator_point().get_position(vector_data)?;
	let direction = (anchor_position - original_c).try_normalize()?;

	let old_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(old_relative_position);
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let handles = [handle, other_handle];
	let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	Some(other_handle)
}

pub fn restore_g1_continuity(
	handle: HandleId,
	other_handle: HandleId,
	control_point: DVec2,
	anchor_position: DVec2,
	vector_data: &VectorData,
	layer: LayerNodeIdentifier,
	responses: &mut VecDeque<Message>,
) {
	let Some(handle_position) = other_handle.to_manipulator_point().get_position(vector_data) else {
		return;
	};
	let Some(direction) = (anchor_position - control_point).try_normalize() else { return };

	let new_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(new_relative_position);
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let handles = [handle, other_handle];
	let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });
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
