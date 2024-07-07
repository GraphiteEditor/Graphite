use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use graphene_std::vector::PointId;

use glam::DVec2;

/// Determines if a path should be extended. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(document: &DocumentMessageHandler, goal: DVec2, tolerance: f64) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	let mut best = None;
	let mut best_distance_squared = tolerance * tolerance;

	for layer in document.selected_nodes.selected_layers(document.network_interface.document_metadata()) {
		let viewspace = document.network_interface.document_metadata().transform_to_viewport(layer);

		let vector_data = document.network_interface.document_metadata().compute_modified_vector(layer, document.network_interface)?;
		for id in vector_data.single_connected_points() {
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
