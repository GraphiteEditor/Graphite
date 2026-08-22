use core_types::attribute::{Attr, EditorLayerPath, RemoveAttr};
use core_types::gpoll::{GraphError, Interrupt};
use core_types::list::List;
use core_types::uuid::NodeId;
use core_types::{ATTR_TRANSFORM, Ctx, ExtractIndex, InjectIndex};
use glam::DAffine2;
use graphic_types::Vector;
use vector_types::markers::EditorClickTarget;
use vector_types::vector::VectorModification;

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
/// The modification applies to the level's first lane only; an unwired input serves one blank lane through the
/// `TypeDefault` value form, so a fresh path starts from a default vector.
#[node_macro::node(category(""))]
fn path_modify<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	(element, existing): (Vector, Attr<'e, EditorLayerPath>),
	modification: Box<VectorModification>,
	node_path: Vec<NodeId>,
) -> Result<(Vector, Attr<'e, EditorLayerPath>, RemoveAttr<EditorClickTarget>), Interrupt> {
	let mut element = element;
	if ctx.innermost_index() == 0 {
		modification.apply(&mut element);
	}

	// Set the path to the encapsulating subgraph (drop our own trailing entry from `node_path`),
	// matching the `path_of_subgraph` proto so editor tools can route data back to the parent layer.
	let path = match existing.is_empty() {
		false => existing.to_vec(),
		true => {
			let len = node_path.len();
			node_path.into_iter().take(len.saturating_sub(1)).collect()
		}
	};
	let (parked, _) = ctx.arena().alloc(path).ok_or(GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_slice()), RemoveAttr::new()))
}

/// Applies the vector path's local transformation to its geometry and resets the transform to the identity.
#[node_macro::node(category("Vector"))]
fn apply_transform(_ctx: impl Ctx, mut vector: List<Vector>) -> List<Vector> {
	let (elements, transforms) = vector.element_and_attribute_slices_mut::<DAffine2>(ATTR_TRANSFORM);
	for (element, transform) in elements.iter_mut().zip(transforms.iter_mut()) {
		for (_, point) in element.point_domain.positions_mut() {
			*point = transform.transform_point2(*point);
		}
		element.segment_domain.transform(*transform);

		*transform = DAffine2::IDENTITY;
	}

	vector
}
