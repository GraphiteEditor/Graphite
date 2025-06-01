use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_text;
use crate::messages::tool::tool_messages::path_tool::PathOverlayMode;
use bezier_rs::{Bezier, TValue};
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

/// Calculates the similarity between two given bezier curves
pub fn calculate_similarity(bezier1: Bezier, bezier2: Bezier, num_samples: usize) -> f64 {
	let points1 = bezier1.compute_lookup_table(Some(num_samples), None).collect::<Vec<_>>();
	let poinst2 = bezier2.compute_lookup_table(Some(num_samples), None).collect::<Vec<_>>();

	let dist = points1.iter().zip(poinst2.iter()).map(|(p1, p2)| (p1.x - p2.x).powi(2) + (p1.y - p2.y).powi(2)).sum::<f64>();
	dist
}

pub fn calculate_similarity_for_given_t(t: f64, p1: DVec2, p2: DVec2, p3: DVec2, d1: DVec2, d2: DVec2, farther_segment: Bezier, other_segment: Bezier) -> (f64, f64, f64) {
	let a = 3. * (1. - t).powi(2) * t;
	let b = 3. * (1. - t) * t.powi(2);

	let rx = p2.x - ((1. - t).powi(3) + 3. * (1. - t).powi(2) * t) * p1.x - (3. * (1. - t) * t.powi(2) + t.powi(3)) * p3.x;
	let ry = p2.y - ((1. - t).powi(3) + 3. * (1. - t).powi(2) * t) * p1.y - (3. * (1. - t) * t.powi(2) + t.powi(3)) * p3.y;

	let det = a * b * (d1.x * d2.y - d1.y * d2.x);

	let start_handle_length = (rx * b * d2.y - ry * b * d2.x) / det;
	let end_handle_length = (ry * a * d1.x - rx * a * d1.y) / det;

	let c1: DVec2 = p1 + d1 * start_handle_length;
	let c2: DVec2 = p3 + d2 * end_handle_length;

	let new_curve = Bezier::from_cubic_coordinates(p1.x, p1.y, c1.x, c1.y, c2.x, c2.y, p3.x, p3.y);
	let [new_first, new_second] = new_curve.split(TValue::Parametric(t));

	//here need a function which calculates the distance between points of the two beziers

	//okay so we need to keep in mind the order of the beziers before sending them to the function
	let new_first = if !(new_first.start.distance(farther_segment.start) < f64::EPSILON) {
		new_first.reverse()
	} else {
		new_first
	};

	let new_second = if !(new_second.start.distance(other_segment.start) < f64::EPSILON) {
		new_second.reverse()
	} else {
		new_second
	};

	let similarity = calculate_similarity(new_first, farther_segment, 10) + calculate_similarity(other_segment, new_second, 10);
	(similarity, start_handle_length, end_handle_length)
}

// Naive approach: Iterates over all t values from
pub fn find_best_approximate(p1: DVec2, p2: DVec2, p3: DVec2, d1: DVec2, d2: DVec2, farther_segment: Bezier, other_segment: Bezier) -> (DVec2, DVec2) {
	let l1 = p2.distance(p1);
	let l2 = p2.distance(p3);
	let approx_t = l1 / (l1 + l2);
	let (mut sim, mut len1, mut len2) = calculate_similarity_for_given_t(approx_t, p1, p2, p3, d1, d2, farther_segment, other_segment);
	let mut valid_segment = len1 > 0. && len2 > 0.;

	for i in 1..100 {
		let t = i as f64 / 100.;
		let (s, li1, li2) = calculate_similarity_for_given_t(t, p1, p2, p3, d1, d2, farther_segment, other_segment);

		if li1 > 0. && li2 > 0. {
			if !valid_segment {
				sim = s;
				len1 = li1;
				len2 = li2;
				valid_segment = true;
			} else if s < sim {
				sim = s;
				len1 = li1;
				len2 = li2;
			}
		}
	}
	(d1 * len1, d2 * len2)
}
