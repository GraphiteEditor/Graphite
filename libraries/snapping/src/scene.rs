use crate::bounds::Bounds;
use crate::bounds_tree::BoundsTree;
use crate::curve::Curve;
use glam::DVec2;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// How many layers may change before the tree over all of them is rebuilt. Until then the changed ones are checked one by one.
const CHANGES_BEFORE_REBUILD: usize = 32;

/// Identifies a layer of the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub u64);

/// Identifies an anchor within its layer, which lets the anchors a gesture is moving be told apart from those staying put.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorId(pub u64);

/// How a piece of geometry relates to what gets edited directly and to what the layer finally draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryRole {
	/// Edited directly, and drawn just as it is.
	Source,
	/// Edited directly, but drawn as something else, so it is there to see only while its layer is being edited.
	Undrawn,
	/// Drawn by the layer without being what is edited.
	Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerKind {
	Artwork,
	Artboard,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneAnchor {
	pub position: DVec2,
	pub id: Option<AnchorId>,
	pub role: GeometryRole,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCurve {
	pub curve: Curve,
	/// The anchors at the start and end of the curve, where they are known.
	pub ends: [Option<AnchorId>; 2],
	pub role: GeometryRole,
}

/// Everything about one layer that can be snapped to, in the coordinates of the work plane.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneLayer {
	pub parent: Option<LayerId>,
	pub kind: LayerKind,
	/// The corners of the layer's bounding box in order around it, oriented with the layer.
	pub quad: Option<[DVec2; 4]>,
	pub anchors: Vec<SceneAnchor>,
	pub curves: Vec<SceneCurve>,
}

impl SceneLayer {
	fn bounds(&self) -> Bounds {
		let quad = self.quad.iter().flatten().copied();
		let anchors = self.anchors.iter().map(|anchor| anchor.position);
		let curves = self.curves.iter().fold(Bounds::EMPTY, |bounds, curve| bounds.union(&curve.curve.bounds()));

		Bounds::from_points(quad.chain(anchors).filter(|point| point.is_finite())).union(&curves)
	}
}

/// What a gesture moves or edits, which decides what can be snapped to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exclusions {
	/// Layers moving as a whole. A moving group lists every layer inside it.
	pub layers: HashSet<LayerId>,
	/// Anchors being moved within a layer that otherwise stays put, which rules out the curves ending at them and counts the layer as edited.
	pub anchors: HashMap<LayerId, HashSet<AnchorId>>,
	/// Layers being edited. Each offers what is edited, drawn or not, but not its output or bounding box, which follow the edit.
	pub edited: HashSet<LayerId>,
}

impl Exclusions {
	pub fn skips_layer(&self, layer: LayerId) -> bool {
		self.layers.contains(&layer)
	}

	pub fn skips_box(&self, layer: LayerId) -> bool {
		self.layers.contains(&layer) || self.is_edited(layer)
	}

	fn is_edited(&self, layer: LayerId) -> bool {
		self.edited.contains(&layer) || self.anchors.contains_key(&layer)
	}

	fn skips_role(&self, layer: LayerId, role: GeometryRole) -> bool {
		let edited = self.is_edited(layer);

		match role {
			GeometryRole::Source => false,
			GeometryRole::Undrawn => !edited,
			GeometryRole::Output => edited,
		}
	}

	fn is_moving(&self, layer: LayerId, anchor: Option<AnchorId>) -> bool {
		anchor.is_some_and(|anchor| self.anchors.get(&layer).is_some_and(|moving| moving.contains(&anchor)))
	}

	pub fn skips_anchor(&self, layer: LayerId, anchor: &SceneAnchor) -> bool {
		self.skips_layer(layer) || self.skips_role(layer, anchor.role) || self.is_moving(layer, anchor.id)
	}

	pub fn skips_curve(&self, layer: LayerId, curve: &SceneCurve) -> bool {
		self.skips_layer(layer) || self.skips_role(layer, curve.role) || curve.ends.iter().any(|&end| self.is_moving(layer, end))
	}
}

#[derive(Clone, Debug)]
struct IndexedLayer {
	layer: SceneLayer,
	bounds: Bounds,
	anchor_tree: BoundsTree,
	curve_tree: BoundsTree,
}

/// The geometry that can be snapped to, indexed so that what lies near a position is found without visiting everything.
///
/// Layers are replaced one at a time as they change. Pointer movement only reads it.
#[derive(Clone, Debug, Default)]
pub struct Scene {
	layers: BTreeMap<LayerId, IndexedLayer>,
	/// A tree over the boxes the layers in `tree_order` had when `prepare` last built it.
	layer_tree: Option<BoundsTree>,
	tree_order: Vec<LayerId>,
	/// The layers set or removed since the tree was built, which it is out of date about.
	changed: BTreeSet<LayerId>,
}

impl Scene {
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a layer, or replaces the one with the same ID. O(n log n) in the layer's own anchors and curves.
	pub fn set_layer(&mut self, id: LayerId, layer: SceneLayer) {
		// An anchor somewhere non-finite is never found, as with a curve
		let anchor_bounds: Vec<Bounds> = layer
			.anchors
			.iter()
			.map(|anchor| if anchor.position.is_finite() { Bounds::from_point(anchor.position) } else { Bounds::EMPTY })
			.collect();
		let curve_bounds: Vec<Bounds> = layer.curves.iter().map(|curve| curve.curve.bounds()).collect();

		let indexed = IndexedLayer {
			bounds: layer.bounds(),
			anchor_tree: BoundsTree::build(&anchor_bounds),
			curve_tree: BoundsTree::build(&curve_bounds),
			layer,
		};
		self.layers.insert(id, indexed);
		self.mark_changed(id);
	}

	pub fn remove_layer(&mut self, id: LayerId) {
		if self.layers.remove(&id).is_some() {
			self.mark_changed(id);
		}
	}

	fn mark_changed(&mut self, id: LayerId) {
		if self.layer_tree.is_some() {
			self.changed.insert(id);
		}
	}

	pub fn clear(&mut self) {
		*self = Self::default();
	}

	pub fn layer(&self, id: LayerId) -> Option<&SceneLayer> {
		self.layers.get(&id).map(|indexed| &indexed.layer)
	}

	pub fn layers(&self) -> impl Iterator<Item = (LayerId, &SceneLayer)> {
		self.layers.iter().map(|(&id, indexed)| (id, &indexed.layer))
	}

	pub fn len(&self) -> usize {
		self.layers.len()
	}

	pub fn is_empty(&self) -> bool {
		self.layers.is_empty()
	}

	/// Rebuilds the tree over the layers once enough of them have changed. O(n log n) in the number of layers when it rebuilds, and nothing otherwise.
	///
	/// Queries are correct without it, but visit every layer until it is first called, and every changed layer after that.
	pub fn prepare(&mut self) {
		if self.layer_tree.is_some() && self.changed.len() <= CHANGES_BEFORE_REBUILD {
			return;
		}

		self.tree_order = self.layers.keys().copied().collect();
		let layer_bounds: Vec<Bounds> = self.layers.values().map(|indexed| indexed.bounds).collect();
		self.layer_tree = Some(BoundsTree::build(&layer_bounds));
		self.changed.clear();
	}

	fn layers_overlapping(&self, area: &Bounds, mut visit: impl FnMut(LayerId, &IndexedLayer)) {
		let Some(layer_tree) = &self.layer_tree else {
			self.layers.iter().filter(|(_, indexed)| indexed.bounds.intersects(area)).for_each(|(&id, indexed)| visit(id, indexed));
			return;
		};

		layer_tree.overlapping(area, |item| {
			if let Some(&id) = self.tree_order.get(item as usize)
				&& !self.changed.contains(&id)
				&& let Some(indexed) = self.layers.get(&id)
			{
				visit(id, indexed);
			}
		});

		for &id in &self.changed {
			if let Some(indexed) = self.layers.get(&id).filter(|indexed| indexed.bounds.intersects(area)) {
				visit(id, indexed);
			}
		}
	}

	/// Visits every anchor the exclusions allow that lies inside an area, with its layer and its index in that layer.
	pub fn anchors_within(&self, area: &Bounds, exclusions: &Exclusions, mut visit: impl FnMut(LayerId, u32, &SceneAnchor)) {
		self.layers_overlapping(area, |id, indexed| {
			if exclusions.skips_layer(id) {
				return;
			}

			indexed.anchor_tree.overlapping(area, |index| {
				let anchor = &indexed.layer.anchors[index as usize];
				if !exclusions.skips_anchor(id, anchor) {
					visit(id, index, anchor);
				}
			});
		});
	}

	/// Visits every curve the exclusions allow that has a box overlapping an area, with its layer and its index in that layer.
	pub fn curves_within(&self, area: &Bounds, exclusions: &Exclusions, mut visit: impl FnMut(LayerId, u32, &SceneCurve)) {
		self.layers_overlapping(area, |id, indexed| {
			if exclusions.skips_layer(id) {
				return;
			}

			indexed.curve_tree.overlapping(area, |index| {
				let curve = &indexed.layer.curves[index as usize];
				if !exclusions.skips_curve(id, curve) {
					visit(id, index, curve);
				}
			});
		});
	}

	/// Visits every layer the exclusions allow a bounding box for that overlaps an area.
	pub fn boxes_within(&self, area: &Bounds, exclusions: &Exclusions, mut visit: impl FnMut(LayerId, &SceneLayer, &[DVec2; 4])) {
		self.layers_overlapping(area, |id, indexed| {
			if let Some(quad) = &indexed.layer.quad
				&& !exclusions.skips_box(id)
				&& Bounds::from_points(*quad).intersects(area)
			{
				visit(id, &indexed.layer, quad);
			}
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn anchor(x: f64, y: f64, id: u64, role: GeometryRole) -> SceneAnchor {
		SceneAnchor {
			position: DVec2::new(x, y),
			id: Some(AnchorId(id)),
			role,
		}
	}

	fn segment(from: (f64, f64), to: (f64, f64), ends: [u64; 2], role: GeometryRole) -> SceneCurve {
		SceneCurve {
			curve: Curve::line(from.into(), to.into()),
			ends: ends.map(|id| Some(AnchorId(id))),
			role,
		}
	}

	/// A square path with anchors numbered around it from its top left corner, which is at `origin`.
	fn square(origin: DVec2, size: f64, role: GeometryRole) -> SceneLayer {
		let corners = [(0., 0.), (size, 0.), (size, size), (0., size)].map(|(x, y)| (origin.x + x, origin.y + y));

		SceneLayer {
			parent: None,
			kind: LayerKind::Artwork,
			quad: Some(corners.map(DVec2::from)),
			anchors: (0..4).map(|index| anchor(corners[index].0, corners[index].1, index as u64, role)).collect(),
			curves: (0..4)
				.map(|index| segment(corners[index], corners[(index + 1) % 4], [index as u64, (index as u64 + 1) % 4], role))
				.collect(),
		}
	}

	/// The anchors and the curves found, each as a layer and an index within it, and the layers that had their boxes found.
	type Found = (Vec<(u64, u32)>, Vec<(u64, u32)>, Vec<u64>);

	fn everything_within(scene: &Scene, area: &Bounds, exclusions: &Exclusions) -> Found {
		let (mut anchors, mut curves, mut boxes) = (Vec::new(), Vec::new(), Vec::new());
		scene.anchors_within(area, exclusions, |layer, index, _| anchors.push((layer.0, index)));
		scene.curves_within(area, exclusions, |layer, index, _| curves.push((layer.0, index)));
		scene.boxes_within(area, exclusions, |layer, _, _| boxes.push(layer.0));

		anchors.sort();
		curves.sort();
		boxes.sort();
		(anchors, curves, boxes)
	}

	fn two_squares() -> Scene {
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), square(DVec2::ZERO, 10., GeometryRole::Source));
		scene.set_layer(LayerId(2), square(DVec2::new(100., 0.), 10., GeometryRole::Source));
		scene
	}

	#[test]
	fn queries_find_only_what_lies_in_the_area() {
		let scene = two_squares();
		let around_a_corner = Bounds::from_point(DVec2::new(10., 10.)).inflated(1.);

		let (anchors, curves, boxes) = everything_within(&scene, &around_a_corner, &Exclusions::default());

		assert_eq!(anchors, vec![(1, 2)]);
		assert_eq!(curves, vec![(1, 1), (1, 2)]);
		assert_eq!(boxes, vec![1]);
	}

	#[test]
	fn queries_agree_before_and_after_the_scene_is_prepared() {
		let mut scene = Scene::new();
		for index in 0..200 {
			let origin = DVec2::new((index % 20) as f64 * 30., (index / 20) as f64 * 30.);
			scene.set_layer(LayerId(index), square(origin, 10., GeometryRole::Source));
		}
		let area = Bounds::from_points([DVec2::new(95., 95.), DVec2::new(160., 130.)]);

		let unprepared = everything_within(&scene, &area, &Exclusions::default());
		scene.prepare();
		let prepared = everything_within(&scene, &area, &Exclusions::default());

		assert!(!prepared.0.is_empty());
		assert_eq!(unprepared, prepared);
	}

	#[test]
	fn queries_stay_right_as_layers_change_after_the_scene_is_prepared() {
		let origin = |index: u64| DVec2::new((index % 20) as f64 * 30., (index / 20) as f64 * 30.);
		let mut scene = Scene::new();
		for index in 0..200 {
			scene.set_layer(LayerId(index), square(origin(index), 10., GeometryRole::Source));
		}
		scene.prepare();
		let area = Bounds::from_points([DVec2::new(95., 95.), DVec2::new(160., 130.)]);

		// Each round moves some layers into the area, removes some, and adds others, then checks against a scene built afresh
		let (mut rebuilt, mut outstanding) = (false, false);
		for round in 0..12 {
			for step in 0..5 {
				let index = round * 17 + step * 3;
				scene.set_layer(LayerId(index), square(DVec2::new(100. + step as f64 * 7., 100. + round as f64), 10., GeometryRole::Source));
				scene.remove_layer(LayerId((64 + index) % 200));
				scene.set_layer(LayerId(1000 + index), square(origin(index) + DVec2::splat(3.), 10., GeometryRole::Source));
			}

			let changed_before = scene.changed.len();
			scene.prepare();
			rebuilt |= scene.changed.len() < changed_before;
			outstanding |= !scene.changed.is_empty();

			let mut afresh = Scene::new();
			for (id, layer) in scene.layers() {
				afresh.set_layer(id, layer.clone());
			}
			assert_eq!(everything_within(&scene, &area, &Exclusions::default()), everything_within(&afresh, &area, &Exclusions::default()));
		}

		// The rounds cover both a tree with changes still outstanding and one rebuilt to take them in
		assert!(rebuilt && outstanding);
	}

	#[test]
	fn replacing_and_removing_layers_is_reflected_at_once() {
		let mut scene = two_squares();
		scene.prepare();
		let area = Bounds::from_point(DVec2::new(105., 5.)).inflated(10.);

		scene.set_layer(LayerId(2), square(DVec2::new(500., 500.), 10., GeometryRole::Source));
		assert!(everything_within(&scene, &area, &Exclusions::default()).2.is_empty());

		scene.set_layer(LayerId(2), square(DVec2::new(100., 0.), 10., GeometryRole::Source));
		assert_eq!(everything_within(&scene, &area, &Exclusions::default()).2, vec![2]);

		scene.remove_layer(LayerId(2));
		scene.prepare();
		assert!(everything_within(&scene, &area, &Exclusions::default()).2.is_empty());
		assert_eq!(scene.len(), 1);
	}

	#[test]
	fn a_layer_moving_as_a_whole_offers_nothing() {
		let scene = two_squares();
		let exclusions = Exclusions {
			layers: HashSet::from([LayerId(1)]),
			..Default::default()
		};

		let (anchors, curves, boxes) = everything_within(&scene, &Bounds::EVERYTHING, &exclusions);

		assert!(anchors.iter().chain(&curves).all(|&(layer, _)| layer == 2));
		assert_eq!(boxes, vec![2]);
	}

	#[test]
	fn moving_anchors_rule_out_themselves_and_the_curves_ending_at_them() {
		let scene = two_squares();
		let exclusions = Exclusions {
			anchors: HashMap::from([(LayerId(1), HashSet::from([AnchorId(2)]))]),
			..Default::default()
		};

		let (anchors, curves, _) = everything_within(&scene, &Bounds::from_points([DVec2::ZERO, DVec2::splat(10.)]), &exclusions);

		// The curves on either side of the moving corner reshape with it, while the other two stay put
		assert_eq!(anchors, vec![(1, 0), (1, 1), (1, 3)]);
		assert_eq!(curves, vec![(1, 0), (1, 3)]);
	}

	#[test]
	fn an_edited_layer_keeps_its_source_as_a_target_but_not_what_it_produces() {
		let mut layer = square(DVec2::ZERO, 10., GeometryRole::Source);
		let produced = square(DVec2::new(20., 0.), 10., GeometryRole::Output);
		layer.anchors.extend(produced.anchors);
		layer.curves.extend(produced.curves);

		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer);
		let exclusions = Exclusions {
			edited: HashSet::from([LayerId(1)]),
			..Default::default()
		};

		let (anchors, curves, boxes) = everything_within(&scene, &Bounds::EVERYTHING, &exclusions);

		assert_eq!(anchors.len(), 4);
		assert_eq!(curves.len(), 4);
		assert!(anchors.iter().chain(&curves).all(|&(_, index)| index < 4));
		assert!(boxes.is_empty());
	}

	#[test]
	fn what_is_edited_but_drawn_as_something_else_is_a_target_only_while_its_layer_is_edited() {
		let mut layer = square(DVec2::ZERO, 10., GeometryRole::Undrawn);
		let drawn = square(DVec2::new(20., 0.), 10., GeometryRole::Output);
		layer.anchors.extend(drawn.anchors);
		layer.curves.extend(drawn.curves);

		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer);

		// Left alone, the layer offers what it draws
		let (anchors, curves, boxes) = everything_within(&scene, &Bounds::EVERYTHING, &Exclusions::default());
		assert_eq!((anchors.len(), curves.len()), (4, 4));
		assert!(anchors.iter().chain(&curves).all(|&(_, index)| index >= 4));
		assert_eq!(boxes, vec![1]);

		// While it is edited, it offers what is being edited instead
		let exclusions = Exclusions {
			edited: HashSet::from([LayerId(1)]),
			..Default::default()
		};
		let (anchors, curves, _) = everything_within(&scene, &Bounds::EVERYTHING, &exclusions);
		assert_eq!((anchors.len(), curves.len()), (4, 4));
		assert!(anchors.iter().chain(&curves).all(|&(_, index)| index < 4));

		// Moving some of its anchors edits it just the same
		let moving_an_anchor = Exclusions {
			anchors: HashMap::from([(LayerId(1), HashSet::from([AnchorId(100)]))]),
			..Default::default()
		};
		assert_eq!(
			everything_within(&scene, &Bounds::EVERYTHING, &moving_an_anchor),
			everything_within(&scene, &Bounds::EVERYTHING, &exclusions)
		);
	}

	#[test]
	fn an_anchor_somewhere_non_finite_hides_none_of_the_others() {
		let mut layer = square(DVec2::ZERO, 10., GeometryRole::Source);
		layer.anchors.push(anchor(f64::NAN, f64::NAN, 4, GeometryRole::Source));
		let mut scene = Scene::new();
		scene.set_layer(LayerId(1), layer.clone());
		let around_a_corner = Bounds::from_point(DVec2::new(10., 10.)).inflated(1.);

		assert_eq!(everything_within(&scene, &around_a_corner, &Exclusions::default()).0, vec![(1, 2)]);
		scene.prepare();
		assert_eq!(everything_within(&scene, &around_a_corner, &Exclusions::default()).0, vec![(1, 2)]);

		// Nor does it empty the box of a layer with nothing else to bound it
		layer.curves.clear();
		layer.quad = None;
		scene.set_layer(LayerId(1), layer);
		assert_eq!(everything_within(&scene, &around_a_corner, &Exclusions::default()).0, vec![(1, 2)]);
	}
}
