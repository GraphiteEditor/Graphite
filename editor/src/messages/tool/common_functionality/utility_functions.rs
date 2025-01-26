use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_text;

use graphene_core::renderer::Quad;
use graphene_core::text::{load_face, FontCache};
use graphene_std::vector::PointId;

use glam::DVec2;

/// Determines if a path should be extended. Goal in viewport space. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(document: &DocumentMessageHandler, goal: DVec2, tolerance: f64, layers: impl Iterator<Item = LayerNodeIdentifier>) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	let mut best = None;
	let mut best_distance_squared = tolerance * tolerance;
	for layer in layers {
		let viewspace = document.metadata().transform_to_viewport(layer);
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
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

/// Gives the bounding box of the text in the layer using max width and height from the typesetting config.
pub fn text_bounding_box(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, font_cache: &FontCache) -> Quad {
	let (text, font, typesetting) = get_text(layer, &document.network_interface).expect("Text layer should have text when interacting with the Text tool");

	let buzz_face = font_cache.get(font).map(|data| load_face(data));
	let far = graphene_core::text::bounding_box(text, buzz_face, typesetting);
	let quad = Quad::from_box([DVec2::ZERO, far]);

	quad
}
