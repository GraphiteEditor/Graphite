use core_types::list::{Item, List};
use core_types::transform::BakeTransform;
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_TRANSFORM, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::Vector;
use vector_types::vector::VectorModification;

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
#[node_macro::node(category(""))]
async fn path_modify(_ctx: impl Ctx, vector: Item<Vector>, modification: Box<VectorModification>, node_path: List<NodeId>) -> Item<Vector> {
	let mut vector = vector;
	modification.apply(vector.element_mut());

	// Drop the stale click-target override so hit testing uses the geometry the user is now editing
	vector.remove_attribute::<Vector>(ATTR_EDITOR_CLICK_TARGET);

	// Set the path to the encapsulating subgraph (drop our own trailing entry from `node_path`),
	// matching the `path_of_subgraph` proto so editor tools can route data back to the parent layer.
	let subgraph_path: List<NodeId> = {
		let len = node_path.len();
		node_path.into_iter().take(len.saturating_sub(1)).collect()
	};
	let existing: List<NodeId> = vector.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH);
	vector.set_attribute(ATTR_EDITOR_LAYER_PATH, if existing.is_empty() { subgraph_path } else { existing });

	vector
}

/// Bakes the content's transform attribute into its underlying value, removing the attribute.
#[node_macro::node(category("Vector"))]
async fn bake_transform<T: BakeTransform + 'n + Send + 'static>(_ctx: impl Ctx, #[implementations(Vector, DAffine2, DVec2)] content: Item<T>) -> Item<T> {
	let mut content = content;
	if let Some(transform) = content.remove_attribute::<DAffine2>(ATTR_TRANSFORM) {
		content.element_mut().bake_transform(&transform);
	}

	content
}
