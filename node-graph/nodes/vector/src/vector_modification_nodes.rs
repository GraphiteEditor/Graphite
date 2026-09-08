use core_types::attribute::{Attr, EditorLayerPath, RemoveAttr, Transform as TransformAttr};
use core_types::gpoll::{GraphError, Interrupt};
use core_types::transform::BakeTransform;
use core_types::uuid::NodeId;
use core_types::{Ctx, ExtractIndex, InjectIndex};
use glam::{DAffine2, DVec2};
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
	if ctx.index() == 0 {
		modification.apply(&mut element);

		// Users draw subpaths in arbitrary winding directions, so normalize them here rather than
		// letting the drawn direction decide fill insideness downstream
		element.normalize_winding_directions();
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

/// Bakes the content's transform attribute into its underlying value, resetting the attribute to the identity.
#[node_macro::node(category("Vector"))]
// Monomorphic on Vector: our macro cannot yet read a record element through an open generic, so master's DAffine2 and DVec2 rows have no node here.
fn bake_transform(_ctx: impl Ctx, (mut content, transform): (Vector, Attr<TransformAttr>)) -> (Vector, Attr<TransformAttr>) {
	let transform: DAffine2 = *transform;
	content.bake_transform(&transform);

	(content, Attr(DAffine2::IDENTITY))
}
