//! Geometry derived from a vector item's point pool and paths, cached on the item so rendering and editor metadata extract it once.

use super::contour::contour_bezpaths;
use super::path::Path;
use super::vector_types::Vector;
use core_types::list::{ATTR_CURVE_TYPE, ATTR_GEOMETRY_CACHE, ATTR_PATHS, AttributeWitness, ElementsWitness, Item, ItemAttributeValues, List};
use glam::DVec2;
use kurbo::BezPath;
use std::sync::{Arc, Mutex};

/// How many geometries one cache remembers, so a memoized item and a modified copy sharing the cache don't keep evicting each other.
const CACHE_ENTRIES: usize = 2;

/// The contours one pool and paths weave, with witnesses to the storage they were extracted from.
struct CachedContours {
	pool: ElementsWitness<DVec2>,
	paths: ElementsWitness<Path>,
	curve_types: AttributeWitness,
	contours: Arc<[BezPath]>,
}

impl CachedContours {
	fn matches(&self, pool: &List<DVec2>, paths: &List<Path>) -> bool {
		self.pool.matches(pool) && self.paths.matches(paths) && self.curve_types.matches(paths, ATTR_CURVE_TYPE)
	}
}

/// A vector item's cache of geometry derived from its pool and paths, stored as its [`ATTR_GEOMETRY_CACHE`] attribute.
/// Clones of the item share it, and it takes no part in equality or hashing.
#[derive(Clone, Default)]
pub struct GeometryCache(Arc<Mutex<Vec<Arc<CachedContours>>>>);

impl GeometryCache {
	/// The contours `vector` and `paths` weave, extracted once and reused for as long as neither changes.
	pub fn contours(&self, vector: &Vector, paths: &List<Path>) -> Arc<[BezPath]> {
		let mut entries = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

		// Most recently used first
		if let Some(position) = entries.iter().position(|entry| entry.matches(&vector.0, paths)) {
			let entry = entries.remove(position);
			let contours = entry.contours.clone();
			entries.insert(0, entry);
			return contours;
		}

		let contours: Arc<[BezPath]> = contour_bezpaths(&vector.0, paths).into();
		let entry = CachedContours {
			pool: vector.0.elements_witness(),
			paths: paths.elements_witness(),
			curve_types: paths.attribute_witness(ATTR_CURVE_TYPE),
			contours: contours.clone(),
		};
		entries.insert(0, Arc::new(entry));
		entries.truncate(CACHE_ENTRIES);

		contours
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
}
