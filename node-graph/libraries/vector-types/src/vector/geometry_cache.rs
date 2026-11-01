//! Geometry derived from a vector item's point pool and paths, cached on the item.

use super::click_target::TargetPath;
use super::contour::contour_bezpaths;
use super::path::Path;
use super::vector_types::{Vector, contours_bounding_box};
use core_types::list::{ATTR_CURVE_TYPE, ATTR_GEOMETRY_CACHE, ATTR_PATHS, AttributeWitness, ElementsWitness, Item, ItemAttributeValues, List};
use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, Stroke, StrokeOpts};
use std::sync::{Arc, Mutex, OnceLock};

/// How many geometries one cache remembers, so a memoized item and a modified copy sharing the cache don't keep evicting each other.
const CACHE_ENTRIES: usize = 2;

/// How many stroke spaces one geometry remembers its placed contours in.
const PLACED_ENTRIES: usize = 4;

/// How many stroke outlines one placement remembers, across stroke styles and zoom levels.
const OUTLINE_ENTRIES: usize = 4;

/// Geometry derived from one pool and paths, each part computed on first use, with witnesses to the storage it derives from.
struct CachedGeometry {
	pool: ElementsWitness<DVec2>,
	paths: ElementsWitness<Path>,
	curve_types: AttributeWitness,
	contours: OnceLock<Arc<[BezPath]>>,
	faces: OnceLock<Arc<[BezPath]>>,
	filled_target_path: OnceLock<Option<Arc<TargetPath>>>,
	unfilled_target_path: OnceLock<Option<Arc<TargetPath>>>,
	bounding_box: OnceLock<Option<[DVec2; 2]>>,
	placed: Mutex<Vec<Arc<PlacedContours>>>,
}

impl CachedGeometry {
	fn matches(&self, pool: &List<DVec2>, paths: &List<Path>) -> bool {
		self.pool.matches(pool) && self.paths.matches(paths) && self.curve_types.matches(paths, ATTR_CURVE_TYPE)
	}

	fn contours(&self, vector: &Vector, paths: &List<Path>) -> &Arc<[BezPath]> {
		self.contours.get_or_init(|| contour_bezpaths(&vector.0, paths).into())
	}

	fn faces(&self, vector: &Vector, paths: &List<Path>) -> &Arc<[BezPath]> {
		self.faces.get_or_init(|| vector.construct_faces(paths).into())
	}

	/// The contours placed by `transform`, kept for the few most recent transforms.
	fn placed(&self, vector: &Vector, paths: &List<Path>, transform: DAffine2) -> Arc<PlacedContours> {
		if let Some(placed) = most_recent(&self.placed, |placed| placed.transform == transform, PLACED_ENTRIES) {
			return placed;
		}

		let affine = Affine::new(transform.to_cols_array());
		let contours = self.contours(vector, paths);
		let mut path = BezPath::from_vec(Vec::with_capacity(contours.iter().map(|contour| contour.elements().len()).sum()));
		for &element in contours.iter().flat_map(|contour| contour.elements()) {
			path.push(affine * element);
		}

		let placed = Arc::new(PlacedContours {
			transform,
			path,
			outlines: Mutex::default(),
		});
		insert_most_recent(&self.placed, placed.clone(), PLACED_ENTRIES);
		placed
	}
}

/// The contours mapped into a stroke's space as one path, with the outlines of the strokes drawn around it.
/// Copies of an item drawn with the same stroke space share these, so their strokes are expanded once.
pub struct PlacedContours {
	transform: DAffine2,
	path: BezPath,
	outlines: Mutex<Vec<Arc<StrokeOutline>>>,
}

struct StrokeOutline {
	stroke: Stroke,
	tolerance_exponent: i32,
	outline: Arc<BezPath>,
}

impl PlacedContours {
	pub fn path(&self) -> &BezPath {
		&self.path
	}

	/// The filled outline of `stroke` around the path, expanded at the power of two at or below `tolerance` so nearby zoom levels share it.
	pub fn stroke_outline(&self, stroke: &Stroke, tolerance: f64) -> Arc<BezPath> {
		let tolerance_exponent = tolerance.log2().floor() as i32;
		let matches = |entry: &Arc<StrokeOutline>| entry.tolerance_exponent == tolerance_exponent && entry.stroke == *stroke;
		if let Some(entry) = most_recent(&self.outlines, matches, OUTLINE_ENTRIES) {
			return entry.outline.clone();
		}

		let outline = Arc::new(kurbo::stroke(self.path.iter(), stroke, &StrokeOpts::default(), 2_f64.powi(tolerance_exponent)));
		let entry = StrokeOutline {
			stroke: stroke.clone(),
			tolerance_exponent,
			outline: outline.clone(),
		};
		insert_most_recent(&self.outlines, Arc::new(entry), OUTLINE_ENTRIES);
		outline
	}
}

/// The entry `matches` picks, moved to the front of the most recently used entries.
fn most_recent<T: ?Sized>(entries: &Mutex<Vec<Arc<T>>>, matches: impl Fn(&Arc<T>) -> bool, limit: usize) -> Option<Arc<T>> {
	let mut entries = entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	let position = entries.iter().position(matches)?;
	let entry = entries.remove(position);
	entries.insert(0, entry.clone());
	entries.truncate(limit);
	Some(entry)
}

/// Adds an entry as the most recently used, evicting the least recently used beyond `limit`.
/// Entries are computed without the lock held, so concurrent readers may each add one, costing only a duplicate.
fn insert_most_recent<T: ?Sized>(entries: &Mutex<Vec<Arc<T>>>, entry: Arc<T>, limit: usize) {
	let mut entries = entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	entries.insert(0, entry);
	entries.truncate(limit);
}

/// A vector item's cache of geometry derived from its pool and paths.
/// Clones of the item share it, and it takes no part in equality or hashing.
#[derive(Clone, Default)]
pub struct GeometryCache(Arc<Mutex<Vec<Arc<CachedGeometry>>>>);

impl GeometryCache {
	/// The contours `vector` and `paths` weave, extracted once and reused for as long as neither changes.
	pub fn contours(&self, vector: &Vector, paths: &List<Path>) -> Arc<[BezPath]> {
		self.entry(vector, paths).contours(vector, paths).clone()
	}

	/// The contours combined into one click target path, built once and reused for as long as neither changes.
	/// When `filled`, it covers what the fill paints: a mesh's faces, or else the contours with the open ones closed.
	pub fn target_path(&self, vector: &Vector, paths: &List<Path>, filled: bool) -> Option<Arc<TargetPath>> {
		let entry = self.entry(vector, paths);
		let slot = if filled { &entry.filled_target_path } else { &entry.unfilled_target_path };

		slot.get_or_init(|| {
			let contours = entry.contours(vector, paths);
			let target_path = if filled && vector.use_face_fill(paths) {
				TargetPath::from_contours_and_faces(contours, entry.faces(vector, paths))
			} else {
				TargetPath::from_contours(contours, filled)
			};
			target_path.map(Arc::new)
		})
		.clone()
	}

	/// The fillable faces of `vector` and `paths`, constructed once and reused for as long as neither changes.
	pub fn faces(&self, vector: &Vector, paths: &List<Path>) -> Arc<[BezPath]> {
		self.entry(vector, paths).faces(vector, paths).clone()
	}

	/// The bounding box of the contours in their own space, computed once and reused for as long as neither changes.
	pub fn bounding_box(&self, vector: &Vector, paths: &List<Path>) -> Option<[DVec2; 2]> {
		let entry = self.entry(vector, paths);
		*entry.bounding_box.get_or_init(|| contours_bounding_box(entry.contours(vector, paths), DAffine2::IDENTITY))
	}

	/// The contours placed in the stroke space given by `transform`, with the stroke outlines expanded around them, reused for as long as neither changes.
	pub fn placed_contours(&self, vector: &Vector, paths: &List<Path>, transform: DAffine2) -> Arc<PlacedContours> {
		self.entry(vector, paths).placed(vector, paths, transform)
	}

	/// The entry for this geometry, made most recently used, or a new empty one evicting the least recently used.
	/// Only the lookup holds the lock, so computing an entry's parts doesn't block other readers of the cache.
	fn entry(&self, vector: &Vector, paths: &List<Path>) -> Arc<CachedGeometry> {
		let mut entries = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

		if let Some(position) = entries.iter().position(|entry| entry.matches(&vector.0, paths)) {
			let entry = entries.remove(position);
			entries.insert(0, entry.clone());
			return entry;
		}

		let entry = Arc::new(CachedGeometry {
			pool: vector.0.elements_witness(),
			paths: paths.elements_witness(),
			curve_types: paths.attribute_witness(ATTR_CURVE_TYPE),
			contours: OnceLock::new(),
			faces: OnceLock::new(),
			filled_target_path: OnceLock::new(),
			unfilled_target_path: OnceLock::new(),
			bounding_box: OnceLock::new(),
			placed: Mutex::default(),
		});
		entries.insert(0, entry.clone());
		entries.truncate(CACHE_ENTRIES);

		entry
	}
}

impl std::fmt::Debug for GeometryCache {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("GeometryCache")
	}
}

// A cache never distinguishes otherwise equal items
impl PartialEq for GeometryCache {
	fn eq(&self, _other: &Self) -> bool {
		true
	}
}

impl graphene_hash::CacheHash for GeometryCache {
	fn cache_hash<H: core::hash::Hasher>(&self, _state: &mut H) {}
}

/// Setting a vector item's paths together with a fresh geometry cache, since new paths mean new geometry.
pub trait SetVectorPaths: Sized {
	/// Sets the item's paths and gives it a fresh geometry cache.
	fn set_paths(&mut self, paths: List<Path>);

	/// The item with the given paths and a fresh geometry cache.
	fn with_paths(mut self, paths: List<Path>) -> Self {
		self.set_paths(paths);
		self
	}
}

impl SetVectorPaths for Item<Vector> {
	fn set_paths(&mut self, paths: List<Path>) {
		self.set_attribute(ATTR_PATHS, paths);
		self.set_attribute(ATTR_GEOMETRY_CACHE, GeometryCache::default());
	}
}

impl SetVectorPaths for ItemAttributeValues {
	fn set_paths(&mut self, paths: List<Path>) {
		self.insert(ATTR_PATHS, paths);
		self.insert(ATTR_GEOMETRY_CACHE, GeometryCache::default());
	}
}

/// Sets the paths of the vector item at `index` together with a fresh geometry cache.
pub fn set_paths_at(list: &mut List<Vector>, index: usize, paths: List<Path>) {
	list.set_attribute(ATTR_PATHS, index, paths);
	list.set_attribute(ATTR_GEOMETRY_CACHE, index, GeometryCache::default());
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::vector::path::CurveType;
	use kurbo::Shape;

	fn square() -> (Vector, List<Path>) {
		Vector::from_bezpath(&kurbo::Rect::new(0., 0., 1., 1.).to_path(0.1))
	}

	#[test]
	fn unchanged_geometry_reuses_its_contours() {
		let (vector, paths) = square();
		let cache = GeometryCache::default();

		let first = cache.contours(&vector, &paths);
		let second = cache.contours(&vector.clone(), &paths.clone());

		assert!(Arc::ptr_eq(&first, &second));
	}

	#[test]
	fn unchanged_geometry_reuses_its_faces() {
		let (vector, paths) = square();
		let cache = GeometryCache::default();

		let first = cache.faces(&vector, &paths);
		let second = cache.faces(&vector, &paths);

		assert!(Arc::ptr_eq(&first, &second));
		assert_eq!(*first, *vector.construct_faces(&paths));
	}

	#[test]
	fn edited_pool_extracts_new_contours() {
		let (mut vector, paths) = square();
		let cache = GeometryCache::default();
		let before = cache.contours(&vector, &paths);

		if let Some(position) = vector.0.element_mut(0) {
			*position = DVec2::new(-1., -1.);
		}
		let after = cache.contours(&vector, &paths);

		assert!(!Arc::ptr_eq(&before, &after));
		assert_eq!(after, contour_bezpaths(&vector.0, &paths).into());
	}

	#[test]
	fn changed_curve_types_extract_new_contours() {
		let (vector, mut paths) = square();
		let cache = GeometryCache::default();
		let before = cache.contours(&vector, &paths);

		paths.set_attribute_values(ATTR_CURVE_TYPE, vec![CurveType::Quadratic; paths.len()]);
		let after = cache.contours(&vector, &paths);

		assert!(!Arc::ptr_eq(&before, &after));
	}

	#[test]
	fn a_shared_cache_holds_two_geometries() {
		let (original, paths) = square();
		let mut edited = original.clone();
		if let Some(position) = edited.0.element_mut(0) {
			*position = DVec2::new(-1., -1.);
		}
		let cache = GeometryCache::default();

		let original_contours = cache.contours(&original, &paths);
		let edited_contours = cache.contours(&edited, &paths);

		assert!(Arc::ptr_eq(&original_contours, &cache.contours(&original, &paths)));
		assert!(Arc::ptr_eq(&edited_contours, &cache.contours(&edited, &paths)));
	}

	#[test]
	fn copies_share_their_stroke_outline() {
		let (vector, paths) = square();
		let cache = GeometryCache::default();
		let stroke = Stroke::new(2.);

		let first = cache.placed_contours(&vector, &paths, DAffine2::IDENTITY).stroke_outline(&stroke, 0.25);
		let copy = cache.clone().placed_contours(&vector.clone(), &paths.clone(), DAffine2::IDENTITY).stroke_outline(&stroke, 0.25);

		assert!(Arc::ptr_eq(&first, &copy));
		assert_eq!(
			*first,
			kurbo::stroke(cache.placed_contours(&vector, &paths, DAffine2::IDENTITY).path().iter(), &stroke, &StrokeOpts::default(), 0.25)
		);
	}

	#[test]
	fn stroke_outlines_are_shared_within_a_power_of_two_of_tolerance() {
		let (vector, paths) = square();
		let placed = GeometryCache::default().placed_contours(&vector, &paths, DAffine2::from_scale(DVec2::splat(10.)));
		let stroke = Stroke::new(2.);

		let coarse = placed.stroke_outline(&stroke, 0.25);
		assert!(Arc::ptr_eq(&coarse, &placed.stroke_outline(&stroke, 0.3)));
		assert!(!Arc::ptr_eq(&coarse, &placed.stroke_outline(&stroke, 0.2)));
		assert!(!Arc::ptr_eq(&coarse, &placed.stroke_outline(&Stroke::new(3.), 0.25)));
	}

	#[test]
	fn placed_contours_follow_their_transform() {
		let (vector, paths) = square();
		let cache = GeometryCache::default();
		let transform = DAffine2::from_scale(DVec2::splat(2.));

		let placed = cache.placed_contours(&vector, &paths, transform);

		assert_eq!(placed.path().bounding_box(), kurbo::Rect::new(0., 0., 2., 2.));
		assert!(!Arc::ptr_eq(&placed, &cache.placed_contours(&vector, &paths, DAffine2::IDENTITY)));
	}
}
