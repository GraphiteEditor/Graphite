use core_types::list::{Item, List, NodeIdPath};
use core_types::transform::BakeTransform;
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_TRANSFORM, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::Vector;
use vector_types::kurbo::BezPath;
use vector_types::vector::{VectorModification, edit_item_geometry};

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
#[node_macro::node(category(""))]
async fn path_modify(_ctx: impl Ctx, vector: Item<Vector>, modification: Item<Box<VectorModification>>, node_path: Item<NodeIdPath>) -> Item<Vector> {
	let mut vector = vector;
	let modification = modification.into_element();
	edit_item_geometry(&mut vector, |element, paths| {
		modification.apply(element, paths);

		// Faces tell negative space apart by a loop's winding, which users draw in arbitrary directions.
		// Every other fill follows the contours, which keep the direction each path was drawn in.
		if element.use_face_fill(paths) {
			element.normalize_winding_directions(paths);
		}
	});

	// Drop the stale click-target override so hit testing uses the geometry the user is now editing
	vector.remove_attribute::<BezPath>(ATTR_EDITOR_CLICK_TARGET);

	// Set the path to the encapsulating subgraph (drop our own trailing entry from `node_path`),
	// matching the `path_of_subgraph` proto so editor tools can route data back to the parent layer.
	let node_path = node_path.into_element().0;
	let subgraph_path: List<NodeId> = {
		let len = node_path.len();
		node_path.into_iter().take(len.saturating_sub(1)).collect()
	};
	let existing = vector.attribute_cloned_or_default::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH).0;
	let layer_path = if existing.is_empty() { subgraph_path } else { existing };
	vector.set_attribute(ATTR_EDITOR_LAYER_PATH, NodeIdPath(layer_path));

	vector
}

/// Bakes the content's transform attribute into its underlying value, removing the attribute.
#[node_macro::node(category("Vector"), icon("NodeBakeTransform"))]
async fn bake_transform<T: BakeTransform + 'n + Send + 'static>(_ctx: impl Ctx, #[implementations(Vector, DAffine2, DVec2)] content: Item<T>) -> Item<T> {
	let mut content = content;
	if let Some(transform) = content.remove_attribute::<DAffine2>(ATTR_TRANSFORM) {
		content.element_mut().bake_transform(&transform);
	}

	content
}

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::vector::paths_from_item_attributes;

	// A lone loop fills by its contour alone, so it keeps the direction it was drawn in even when that winds backwards
	#[tokio::test]
	async fn a_drawn_loop_keeps_its_direction() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((0., 10.));
		bezpath.line_to((10., 10.));
		bezpath.line_to((10., 0.));
		bezpath.close_path();
		let drawn = Vector::item_from_bezpath(&bezpath);

		let modified = path_modify((), drawn.clone(), Item::default(), Item::default()).await;

		assert_eq!(paths_from_item_attributes(&modified), paths_from_item_attributes(&drawn));
	}
}
