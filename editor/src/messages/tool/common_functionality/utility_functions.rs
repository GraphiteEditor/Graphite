use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use graphene_std::vector::PointId;

use glam::DVec2;

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
