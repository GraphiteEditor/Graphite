use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_subpaths;
use glam::DVec2;
use graphene_std::vector::PointId;

/// Determines if a path should be extended. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(document: &DocumentMessageHandler, goal: DVec2, tolerance: f64) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	let mut best = None;
	let mut best_distance_squared = tolerance * tolerance;

	for layer in document.selected_nodes.selected_layers(document.metadata()) {
		let viewspace = document.metadata().transform_to_viewport(layer);

		let vector_data = document.metadata.compute_modified_vector(layer, document.network())?;
		for id in vector_data.single_connected_points() {
			let Some(point) = vector_data.point_domain.pos_from_id(id) else { continue };

			let distance_squared = viewspace.transform_point2(point).distance_squared(goal);

			if distance_squared < best_distance_squared {
				best = Some((layer, id, point));
				best_distance_squared = distance_squared;
			}
		}
	}

	best
}
