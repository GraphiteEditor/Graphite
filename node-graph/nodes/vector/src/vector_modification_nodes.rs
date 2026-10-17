use core_types::list::{Item, List, NodeIdPath};
use core_types::transform::BakeTransform;
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_TRANSFORM, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::Vector;
use vector_types::kurbo::BezPath;
use vector_types::vector::{VectorModification, edit_item_geometry, ensure_paths_attribute, shared_paths_from_item_attributes};

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
#[node_macro::node(category(""))]
async fn path_modify(_ctx: impl Ctx, vector: Item<Vector>, modification: Item<Box<VectorModification>>, node_path: Item<NodeIdPath>) -> Item<Vector> {
	let mut vector = vector;
	let modification = modification.into_element();

	// Moving points leaves the paths as they are, so they stay shared with the incoming geometry instead of being copied
	let paths = shared_paths_from_item_attributes(&vector);
	let moved_in_place = modification.apply_in_place(vector.element_mut(), &paths);
	let fills_by_face = moved_in_place && vector.element().use_face_fill(&paths);
	drop(paths);

	if moved_in_place && !fills_by_face {
		ensure_paths_attribute(&mut vector);
	} else {
		edit_item_geometry(&mut vector, |element, paths| {
			if !moved_in_place {
				modification.apply(element, paths);
			}

			// Faces tell negative space apart by a loop's winding, which users draw in arbitrary directions.
			// Every other fill follows the contours, which keep the direction each path was drawn in.
			if fills_by_face || (!moved_in_place && element.use_face_fill(paths)) {
				element.normalize_winding_directions(paths);
			}
		});
	}

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
	use core_types::list::{ATTR_ID, ATTR_PATHS};
	use vector_types::vector::{CurveType, PointId, SegmentId, VectorModificationType, paths_from_item_attributes, push_path};

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

	// Moving a point of a mesh leaves its paths in place to begin with, and its loops still get their winding set for the face fill
	#[tokio::test]
	async fn a_mesh_moved_in_place_still_has_its_winding_set() {
		// A reverse-wound square beside three spokes meeting at a hub, which makes the geometry branch
		let positions = [(0., 0.), (0., 10.), (10., 10.), (10., 0.), (30., 0.), (40., 0.), (30., 10.), (20., 10.)];
		let (mut vector, mut paths) = (Vector::default(), List::new());
		let (mut point_id, mut segment_id) = (PointId::ZERO, SegmentId::ZERO);
		for (x, y) in positions {
			vector.0.push(Item::new_from_element(DVec2::new(x, y)).with_attribute(ATTR_ID, point_id.next_id()));
		}
		for references in [vec![0, 1, 2, 3, 0], vec![4, 5], vec![4, 6], vec![4, 7]] {
			let ids: Vec<SegmentId> = (1..references.len()).map(|_| segment_id.next_id()).collect();
			push_path(&mut paths, CurveType::Linear, references, ids);
		}
		assert!(vector.use_face_fill(&paths));
		let hub = vector.point_id_at(4).unwrap();
		let drawn = Item::new_from_element(vector).with_attribute(ATTR_PATHS, paths);

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::ApplyPointDelta { point: hub, delta: DVec2::X });
		let modified = path_modify((), drawn.clone(), Item::new_from_element(Box::new(modification)), Item::default()).await;

		let first_path = |item: &Item<Vector>| paths_from_item_attributes(item).unwrap().element(0).unwrap().0.iter_element_values().copied().collect::<Vec<_>>();
		assert_eq!(first_path(&drawn), [0, 1, 2, 3, 0]);
		assert_eq!(first_path(&modified), [0, 3, 2, 1, 0]);
	}

	// Moving points leaves the paths shared with the incoming geometry, and where they sat among its attributes
	#[tokio::test]
	async fn moving_points_leaves_the_paths_shared_and_in_place() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((10., 0.));
		bezpath.line_to((10., 10.));
		let drawn = Vector::item_from_bezpath(&bezpath).with_attribute(ATTR_TRANSFORM, DAffine2::IDENTITY);
		let corner = drawn.element().point_id_at(1).unwrap();

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::ApplyPointDelta { point: corner, delta: DVec2::X });
		let modified = path_modify((), drawn.clone(), Item::new_from_element(Box::new(modification)), Item::default()).await;

		assert_eq!(modified.element().0.element(1), Some(&DVec2::new(11., 0.)));
		assert!(std::ptr::eq(paths_from_item_attributes(&drawn).unwrap(), paths_from_item_attributes(&modified).unwrap()));
		assert_eq!(modified.attributes().keys().collect::<Vec<_>>(), [ATTR_PATHS, ATTR_TRANSFORM, ATTR_EDITOR_LAYER_PATH]);
	}

	// A point cloud comes out with an empty paths attribute placed last, whether the edit is made in place or by reweaving
	#[tokio::test]
	async fn a_point_cloud_gains_an_empty_paths_attribute() {
		let mut cloud = Vector::default();
		cloud.0.push(Item::new_from_element(DVec2::ZERO).with_attribute(ATTR_ID, PointId::ZERO));
		let drawn = Item::new_from_element(cloud).with_attribute(ATTR_TRANSFORM, DAffine2::IDENTITY);

		let mut adds_a_point = VectorModification::default();
		adds_a_point.modify(&VectorModificationType::InsertPoint {
			id: PointId::generate(),
			position: DVec2::ONE,
		});

		for modification in [VectorModification::default(), adds_a_point] {
			let modified = path_modify((), drawn.clone(), Item::new_from_element(Box::new(modification)), Item::default()).await;

			assert_eq!(paths_from_item_attributes(&modified).map(List::len), Some(0));
			assert_eq!(modified.attributes().keys().collect::<Vec<_>>(), [ATTR_TRANSFORM, ATTR_PATHS, ATTR_EDITOR_LAYER_PATH]);
		}
	}
}
