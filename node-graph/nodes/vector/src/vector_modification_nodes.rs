use core_types::Ctx;
use core_types::table::Table;
use core_types::uuid::NodeId;
use glam::DAffine2;
use graphic_types::Vector;
use vector_types::vector::VectorModification;

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
#[node_macro::node(category(""))]
async fn path_modify(_ctx: impl Ctx, mut vector: Table<Vector>, modification: Box<VectorModification>, node_path: Table<NodeId>) -> Table<Vector> {
	use core_types::table::TableRow;

	if vector.is_empty() {
		vector.push(TableRow::default());
	}
	modification.apply(vector.element_mut(0).expect("push should give one item"));

	// Set the path to the encapsulating subgraph (drop our own trailing entry from `node_path`),
	// matching the `path_of_subgraph` proto so editor tools can route data back to the parent layer.
	let subgraph_path: Table<NodeId> = {
		let len = node_path.len();
		node_path.into_iter().take(len.saturating_sub(1)).collect()
	};
	let existing: Table<NodeId> = vector.attribute_cloned_or_default("editor:layer", 0);
	vector.set_attribute("editor:layer", 0, if existing.is_empty() { subgraph_path } else { existing });

	if vector.len() > 1 {
		warn!("The path modify ran on {} vector items. Only the first can be modified.", vector.len());
	}
	vector
}

/// Applies the vector path's local transformation to its geometry and resets the transform to the identity.
#[node_macro::node(category("Vector"))]
async fn apply_transform(_ctx: impl Ctx, mut vector: Table<Vector>) -> Table<Vector> {
	let (elements, transforms) = vector.element_and_attribute_slices_mut::<DAffine2>("transform");
	for (element, transform) in elements.iter_mut().zip(transforms.iter_mut()) {
		for (_, point) in element.point_domain.positions_mut() {
			*point = transform.transform_point2(*point);
		}
		element.segment_domain.transform(*transform);

		*transform = DAffine2::IDENTITY;
	}

	vector
}
