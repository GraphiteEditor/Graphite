use core_types::Ctx;
use core_types::table::Table;
use core_types::uuid::NodeId;
use glam::DAffine2;
use graphic_types::Vector;
use vector_types::vector::VectorModification;

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
#[node_macro::node(category(""))]
async fn path_modify(_ctx: impl Ctx, mut vector: Table<Vector>, modification: Box<VectorModification>, node_path: Vec<NodeId>) -> Table<Vector> {
	use core_types::table::TableRow;

	if vector.is_empty() {
		vector.push(TableRow::default());
	}
	let row = vector.get_mut(0).expect("push should give one item");
	modification.apply(row.element);

	// Update the source node id
	let this_node_path = node_path.iter().rev().nth(1).copied();
	*row.source_node_id = row.source_node_id.or(this_node_path);

	if vector.len() > 1 {
		warn!("The path modify ran on {} vector rows. Only the first can be modified.", vector.len());
	}
	vector
}

/// Applies the vector path's local transformation to its geometry and resets the transform to the identity.
#[node_macro::node(category("Vector"))]
async fn apply_transform(_ctx: impl Ctx, mut vector: Table<Vector>) -> Table<Vector> {
	for row in vector.iter_mut() {
		let vector = row.element;
		let transform = *row.transform;

		for (_, point) in vector.point_domain.positions_mut() {
			*point = transform.transform_point2(*point);
		}
		vector.segment_domain.transform(transform);

		*row.transform = DAffine2::IDENTITY;
	}

	vector
}
