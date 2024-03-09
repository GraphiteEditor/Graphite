use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_subpaths;
use glam::DVec2;

/// Determines if a path should be extended. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(document: &DocumentMessageHandler, pos: DVec2, tolerance: f64) -> Option<(LayerNodeIdentifier, usize, bool)> {
	let mut best = None;
	let mut best_distance_squared = tolerance * tolerance;

	for layer in document.selected_nodes.selected_layers(document.metadata()) {
		let viewspace = document.metadata().transform_to_viewport(layer);

		let subpaths = get_subpaths(layer, &document.network)?;
		for (subpath_index, subpath) in subpaths.iter().enumerate() {
			if subpath.closed() {
				continue;
			}

			for (manipulator_group, from_start) in [(subpath.manipulator_groups().first(), true), (subpath.manipulator_groups().last(), false)] {
				let Some(manipulator_group) = manipulator_group else { break };

				let distance_squared = viewspace.transform_point2(manipulator_group.anchor).distance_squared(pos);

				if distance_squared < best_distance_squared {
					best = Some((layer, subpath_index, from_start));
					best_distance_squared = distance_squared;
				}
			}
		}
	}

	best
}
