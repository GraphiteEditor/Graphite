use std::sync::{Arc, RwLock};

use super::algorithms::{bezpath_algorithms::bezpath_is_inside_bezpath, intersection::filtered_segment_intersections};
use super::misc::dvec2_to_point;
use crate::math::QuadExt;
use crate::vector::PointId;
use core_types::math::quad::Quad;
use core_types::transform::Transform;
use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, ParamCurve, PathEl, PathSeg, Shape};

type BoundingBox = Option<[DVec2; 2]>;

/// Per-segment tight bounding box union of the transformed path, or None if the path has no segments.
fn bezpath_bounding_box_with_transform(bezpath: &BezPath, transform: DAffine2) -> BoundingBox {
	let affine = Affine::new(transform.to_cols_array());
	bezpath
		.segments()
		.map(|segment| (affine * segment).bounding_box())
		.reduce(|a, b| a.union(b))
		.map(|rect| [DVec2::new(rect.min_x(), rect.min_y()), DVec2::new(rect.max_x(), rect.max_y())])
}

/// The explicitly closed contours of the path, which together form its fillable region.
fn closed_contours(bezpath: &BezPath) -> BezPath {
	let elements = bezpath.elements();
	let mut kept = Vec::new();
	let mut contour_start = 0;

	for (index, element) in elements.iter().enumerate() {
		if matches!(element, PathEl::MoveTo(_)) {
			contour_start = index;
		}
		if matches!(element, PathEl::ClosePath) {
			kept.extend_from_slice(&elements[contour_start..=index]);
		}
	}

	BezPath::from_vec(kept)
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreePoint {
	pub id: PointId,
	pub position: DVec2,
}

impl FreePoint {
	pub fn new(id: PointId, position: DVec2) -> Self {
		Self { id, position }
	}

	pub fn apply_transform(&mut self, transform: DAffine2) {
		self.position = transform.transform_point2(self.position);
	}
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClickTargetType {
	/// One or more contours tested as one compound shape using the non-zero fill rule, so holes
	/// (e.g. the inside of an "O") correctly count as outside the fill.
	Path(BezPath),
	FreePoint(FreePoint),
}

/// Fixed-size ring buffer cache for rotated bounding boxes.
///
/// Stores up to 8 rotation angles and their corresponding bounding boxes to avoid
/// recomputing expensive bezier curve bounds for repeated rotations. Uses 7-bit
/// fingerprint hashing with MSB as presence flag for fast lookup.
#[derive(Clone, Debug, Default)]
struct BoundingBoxCache {
	/// Packed 7-bit fingerprints with MSB presence flags for cache lookup
	fingerprints: u64,
	/// (rotation_angle, cached_bounds) pairs
	elements: [(f64, BoundingBox); Self::CACHE_SIZE],
	/// Next position to write in ring buffer
	write_ptr: usize,
}

impl BoundingBoxCache {
	/// Cache size - must be ≤ 8 since fingerprints is u64 (8 bytes, 1 byte per element)
	const CACHE_SIZE: usize = 8;
	const FINGERPRINT_BITS: u32 = 7;
	const PRESENCE_FLAG: u8 = 1 << Self::FINGERPRINT_BITS;

	/// Generates a 7-bit fingerprint from rotation with MSB as presence flag
	fn rotation_fingerprint(rotation: f64) -> u8 {
		(rotation.to_bits() % (1 << Self::FINGERPRINT_BITS)) as u8 | Self::PRESENCE_FLAG
	}
	/// Attempts to find cached bounding box for the given rotation.
	/// Returns Some(bounds) if found, None if not cached.
	fn try_read(&self, rotation: f64, scale: DVec2, translation: DVec2, fingerprint: u8) -> Option<BoundingBox> {
		// Build bitmask of positions with matching fingerprints for vectorized comparison
		let mut mask: u8 = 0;
		for (i, fp) in (0..Self::CACHE_SIZE).zip(self.fingerprints.to_le_bytes()) {
			// Check MSB for presence and lower 7 bits for fingerprint match
			if fp == fingerprint {
				mask |= 1 << i;
			}
		}
		// Check each position with matching fingerprint for exact rotation match
		while mask != 0 {
			let pos = mask.trailing_zeros() as usize;

			if rotation == self.elements[pos].0 {
				// Found cached rotation - apply scale and translation to cached bounds
				let transform = DAffine2::from_scale_angle_translation(scale, 0., translation);
				let new_bounds = self.elements[pos].1.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)]);

				return Some(new_bounds);
			}
			mask &= !(1 << pos);
		}
		None
	}
	/// Computes and caches bounding box for the given rotation, then applies scale/translation.
	/// Returns the final transformed bounds.
	fn add_to_cache(&mut self, bezpath: &BezPath, rotation: f64, scale: DVec2, translation: DVec2, fingerprint: u8) -> BoundingBox {
		// Compute bounds for pure rotation (expensive operation we want to cache)
		let bounds = bezpath_bounding_box_with_transform(bezpath, DAffine2::from_angle(rotation));

		if bounds.is_none() {
			return bounds;
		}

		// Store in ring buffer at current write position
		let write_ptr = self.write_ptr;
		self.elements[write_ptr] = (rotation, bounds);

		// Update fingerprint byte for this position
		let mut bytes = self.fingerprints.to_le_bytes();
		bytes[write_ptr] = fingerprint;
		self.fingerprints = u64::from_le_bytes(bytes);

		// Advance write pointer (ring buffer behavior)
		self.write_ptr = (write_ptr + 1) % Self::CACHE_SIZE;

		// Apply scale and translation to cached rotated bounds
		let transform = DAffine2::from_scale_angle_translation(scale, 0., translation);
		bounds.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)])
	}
}

/// Represents a clickable target for the layer
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClickTarget {
	target_type: ClickTargetType,
	stroke_width: f64,
	bounding_box: BoundingBox,
	#[cfg_attr(feature = "serde", serde(skip))]
	bounding_box_cache: Arc<RwLock<BoundingBoxCache>>,
}

impl PartialEq for ClickTarget {
	fn eq(&self, other: &Self) -> bool {
		self.target_type == other.target_type && self.stroke_width == other.stroke_width && self.bounding_box == other.bounding_box
	}
}

impl ClickTarget {
	pub fn new_with_path(path: BezPath, stroke_width: f64) -> Self {
		// The control-point hull serves as the loose bounding box
		let control_box = path.control_box();
		let bounding_box = (!path.elements().is_empty()).then(|| [DVec2::new(control_box.min_x(), control_box.min_y()), DVec2::new(control_box.max_x(), control_box.max_y())]);
		Self {
			target_type: ClickTargetType::Path(path),
			stroke_width,
			bounding_box,
			bounding_box_cache: Default::default(),
		}
	}

	pub fn new_with_free_point(point: FreePoint) -> Self {
		const MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT: f64 = 1e-4 / 2.;
		let stroke_width = 10.;
		let bounding_box = Some([
			point.position - DVec2::splat(MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT),
			point.position + DVec2::splat(MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT),
		]);

		Self {
			target_type: ClickTargetType::FreePoint(point),
			stroke_width,
			bounding_box,
			bounding_box_cache: Default::default(),
		}
	}

	pub fn target_type(&self) -> &ClickTargetType {
		&self.target_type
	}

	pub fn bounding_box(&self) -> BoundingBox {
		self.bounding_box
	}

	pub fn bounding_box_center(&self) -> Option<DVec2> {
		self.bounding_box.map(|bbox| bbox[0] + (bbox[1] - bbox[0]) / 2.)
	}

	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> BoundingBox {
		match self.target_type {
			ClickTargetType::Path(ref path) => {
				// Bypass cache for skewed transforms since rotation decomposition isn't valid
				if transform.has_skew() {
					return bezpath_bounding_box_with_transform(path, transform);
				}

				// Decompose transform into rotation, scale, translation for caching strategy
				let rotation = transform.decompose_rotation();
				let scale = transform.scale_magnitudes();
				let translation = transform.translation;

				// Generate fingerprint for cache lookup
				let fingerprint = BoundingBoxCache::rotation_fingerprint(rotation);

				// Try to read from cache first
				let read_lock = self.bounding_box_cache.read().unwrap();
				if let Some(value) = read_lock.try_read(rotation, scale, translation, fingerprint) {
					return value;
				}
				std::mem::drop(read_lock);

				// Cache miss - compute and store new entry
				let mut write_lock = self.bounding_box_cache.write().unwrap();
				write_lock.add_to_cache(path, rotation, scale, translation, fingerprint)
			}
			// TODO: use point for calculation of bbox
			ClickTargetType::FreePoint(_) => self.bounding_box.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)]),
		}
	}

	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		match self.target_type {
			ClickTargetType::Path(ref mut path) => {
				path.apply_affine(Affine::new(affine_transform.to_cols_array()));
			}
			ClickTargetType::FreePoint(ref mut point) => {
				point.apply_transform(affine_transform);
			}
		}
		self.update_bbox();
	}

	fn update_bbox(&mut self) {
		match self.target_type {
			ClickTargetType::Path(ref path) => {
				self.bounding_box = bezpath_bounding_box_with_transform(path, DAffine2::IDENTITY);
			}
			ClickTargetType::FreePoint(ref point) => {
				self.bounding_box = Some([point.position - DVec2::splat(self.stroke_width / 2.), point.position + DVec2::splat(self.stroke_width / 2.)]);
			}
		}
	}

	/// Does the click target intersect the path
	pub fn intersect_path<It: Iterator<Item = PathSeg>>(&self, mut bezier_iter: impl FnMut() -> It, layer_transform: DAffine2) -> bool {
		// Check if the matrix is not invertible
		let mut layer_transform = layer_transform;
		if layer_transform.matrix2.determinant().abs() <= f64::EPSILON {
			layer_transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		}

		let inverse = layer_transform.inverse();
		let mut bezier_iter = || bezier_iter().map(|bezier| Affine::new(inverse.to_cols_array()) * bezier);

		match self.target_type() {
			ClickTargetType::Path(path) => {
				// Outline intersection (catches strokes and both filled/unfilled shapes)
				let outline_intersects = |path_segment: PathSeg| bezier_iter().any(|line| !filtered_segment_intersections(path_segment, line, None, None).is_empty());
				if path.segments().any(outline_intersects) {
					return true;
				}

				// Selection point inside the fill (non-zero rule).
				// Only closed contours contribute to the fill region; open segments would otherwise produce spurious winding on one side of the segment.
				let fill_region = closed_contours(path);
				if !fill_region.is_empty() && bezier_iter().next().is_some_and(|segment| fill_region.contains(segment.start())) {
					return true;
				}

				// Build closed selection path, then check if the whole shape is entirely within it
				let mut selection = BezPath::from_path_segments(bezier_iter());
				selection.close_path();
				bezpath_is_inside_bezpath(path, &selection, None, None)
			}
			ClickTargetType::FreePoint(point) => bezier_iter().map(|segment: PathSeg| segment.winding(dvec2_to_point(point.position))).sum::<i32>() != 0,
		}
	}

	/// Does the click target intersect the point (accounting for stroke size)
	pub fn intersect_point(&self, point: DVec2, layer_transform: DAffine2) -> bool {
		let target_bounds = [point - DVec2::splat(self.stroke_width / 2.), point + DVec2::splat(self.stroke_width / 2.)];
		let intersects = |a: [DVec2; 2], b: [DVec2; 2]| a[0].x <= b[1].x && a[1].x >= b[0].x && a[0].y <= b[1].y && a[1].y >= b[0].y;
		// This bounding box is not very accurate as it is the axis aligned version of the transformed bounding box. However it is fast.
		if !self
			.bounding_box
			.is_some_and(|loose| (loose[0] - loose[1]).abs().cmpgt(DVec2::splat(1e-4)).any() && intersects((layer_transform * Quad::from_box(loose)).bounding_box(), target_bounds))
		{
			return false;
		}

		// Allows for selecting lines
		// TODO: actual intersection of stroke
		let inflated_quad = Quad::from_box(target_bounds);
		self.intersect_path(|| inflated_quad.to_lines(), layer_transform)
	}

	/// Does the click target intersect the point (not accounting for stroke size)
	pub fn intersect_point_no_stroke(&self, point: DVec2) -> bool {
		// Check if the point is within the bounding box
		if self
			.bounding_box
			.is_some_and(|bbox| bbox[0].x <= point.x && point.x <= bbox[1].x && bbox[0].y <= point.y && point.y <= bbox[1].y)
		{
			// Check if the point is within the shape
			match self.target_type() {
				ClickTargetType::Path(path) => path.contains(dvec2_to_point(point)),
				ClickTargetType::FreePoint(free_point) => free_point.position == point,
			}
		} else {
			false
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::DVec2;
	use kurbo::{DEFAULT_ACCURACY, Rect};
	use std::f64::consts::PI;

	fn rectangle_path(corner1: DVec2, corner2: DVec2) -> BezPath {
		Rect::new(corner1.x, corner1.y, corner2.x, corner2.y).to_path(DEFAULT_ACCURACY)
	}

	#[test]
	fn test_bounding_box_cache_fingerprint_generation() {
		// Test that fingerprints have MSB set and use only 7 bits for data
		let rotation1 = 0.;
		let rotation2 = PI / 3.;
		let rotation3 = PI / 2.;

		let fp1 = BoundingBoxCache::rotation_fingerprint(rotation1);
		let fp2 = BoundingBoxCache::rotation_fingerprint(rotation2);
		let fp3 = BoundingBoxCache::rotation_fingerprint(rotation3);

		// All fingerprints should have MSB set (presence flag)
		assert_eq!(fp1 & BoundingBoxCache::PRESENCE_FLAG, BoundingBoxCache::PRESENCE_FLAG);
		assert_eq!(fp2 & BoundingBoxCache::PRESENCE_FLAG, BoundingBoxCache::PRESENCE_FLAG);
		assert_eq!(fp3 & BoundingBoxCache::PRESENCE_FLAG, BoundingBoxCache::PRESENCE_FLAG);

		// Lower 7 bits should contain the actual fingerprint data
		let data1 = fp1 & !BoundingBoxCache::PRESENCE_FLAG;
		let data2 = fp2 & !BoundingBoxCache::PRESENCE_FLAG;
		let data3 = fp3 & !BoundingBoxCache::PRESENCE_FLAG;

		// Data portions should be different (unless collision)
		assert!(data1 != data2 && data2 != data3 && data3 != data1);
	}

	#[test]
	fn test_bounding_box_cache_basic_operations() {
		let mut cache = BoundingBoxCache::default();

		// Create a simple rectangle path for testing
		let path = rectangle_path(DVec2::ZERO, DVec2::new(100., 50.));

		let rotation = PI / 4.;
		let scale = DVec2::new(2., 2.);
		let translation = DVec2::new(10., 20.);
		let fingerprint = BoundingBoxCache::rotation_fingerprint(rotation);

		// Cache should be empty initially
		assert!(cache.try_read(rotation, scale, translation, fingerprint).is_none());

		// Add to cache
		let result = cache.add_to_cache(&path, rotation, scale, translation, fingerprint);
		assert!(result.is_some());

		// Should now be able to read from cache
		let cached = cache.try_read(rotation, scale, translation, fingerprint);
		assert!(cached.is_some());
		assert_eq!(cached.unwrap(), result);
	}

	#[test]
	fn test_bounding_box_cache_ring_buffer_behavior() {
		let mut cache = BoundingBoxCache::default();
		let path = rectangle_path(DVec2::ZERO, DVec2::new(10., 10.));
		let scale = DVec2::ONE;
		let translation = DVec2::ZERO;

		// Fill cache beyond capacity to test ring buffer behavior
		let rotations: Vec<f64> = (0..10).map(|i| i as f64 * PI / 8.).collect();

		for rotation in &rotations {
			let fingerprint = BoundingBoxCache::rotation_fingerprint(*rotation);
			cache.add_to_cache(&path, *rotation, scale, translation, fingerprint);
		}

		// First two entries should be overwritten (cache size is 8)
		let first_fp = BoundingBoxCache::rotation_fingerprint(rotations[0]);
		let second_fp = BoundingBoxCache::rotation_fingerprint(rotations[1]);
		let last_fp = BoundingBoxCache::rotation_fingerprint(rotations[9]);

		assert!(cache.try_read(rotations[0], scale, translation, first_fp).is_none());
		assert!(cache.try_read(rotations[1], scale, translation, second_fp).is_none());
		assert!(cache.try_read(rotations[9], scale, translation, last_fp).is_some());
	}

	#[test]
	fn test_click_target_bounding_box_caching() {
		// Create a click target with a simple rectangle
		let path = rectangle_path(DVec2::ZERO, DVec2::new(100., 50.));
		let click_target = ClickTarget::new_with_path(path, 1.);

		let rotation = PI / 6.;
		let scale = DVec2::new(1.5, 1.5);
		let translation = DVec2::new(20., 30.);
		let transform = DAffine2::from_scale_angle_translation(scale, rotation, translation);

		// Helper function to count present values in cache
		let count_present_values = || {
			let cache = click_target.bounding_box_cache.read().unwrap();
			cache.fingerprints.to_le_bytes().iter().filter(|&&fp| fp & BoundingBoxCache::PRESENCE_FLAG != 0).count()
		};

		// Initially cache should be empty
		assert_eq!(count_present_values(), 0);

		// First call should compute and cache
		let result1 = click_target.bounding_box_with_transform(transform);
		assert!(result1.is_some());
		assert_eq!(count_present_values(), 1);

		// Second call with same transform should use cache, not add new entry
		let result2 = click_target.bounding_box_with_transform(transform);
		assert_eq!(result1, result2);
		assert_eq!(count_present_values(), 1); // Should still be 1, not 2

		// Different scale/translation but same rotation should use cached rotation
		let transform2 = DAffine2::from_scale_angle_translation(DVec2::new(2., 2.), rotation, DVec2::new(50., 60.));
		let result3 = click_target.bounding_box_with_transform(transform2);
		assert!(result3.is_some());
		assert_ne!(result1, result3); // Different due to different scale/translation
		assert_eq!(count_present_values(), 1); // Should still be 1, reused same rotation
	}

	#[test]
	fn test_click_target_skew_bypass_cache() {
		let path = rectangle_path(DVec2::ZERO, DVec2::new(100., 50.));
		let click_target = ClickTarget::new_with_path(path.clone(), 1.);

		// Create a transform with skew (non-uniform scaling in different directions)
		let skew_transform = DAffine2::from_cols_array(&[2., 0.5, 0., 1., 10., 20.]);
		assert!(skew_transform.has_skew());

		// Should bypass cache and compute directly
		let result = click_target.bounding_box_with_transform(skew_transform);
		let expected = bezpath_bounding_box_with_transform(&path, skew_transform);
		assert_eq!(result, expected);
	}

	#[test]
	fn test_cache_fingerprint_collision_handling() {
		let mut cache = BoundingBoxCache::default();
		let path = rectangle_path(DVec2::ZERO, DVec2::new(10., 10.));
		let scale = DVec2::ONE;
		let translation = DVec2::ZERO;

		// Find two rotations that produce the same fingerprint (collision)
		let rotation1 = 0.;
		let rotation2 = 0.25;
		let fp1 = BoundingBoxCache::rotation_fingerprint(rotation1);
		let fp2 = BoundingBoxCache::rotation_fingerprint(rotation2);

		// If we found a collision, test that exact rotation matching still works
		if fp1 == fp2 && rotation1 != rotation2 {
			// Add first rotation
			cache.add_to_cache(&path, rotation1, scale, translation, fp1);

			// Should find the exact rotation
			assert!(cache.try_read(rotation1, scale, translation, fp1).is_some());

			// Should not find the colliding rotation (different exact value)
			assert!(cache.try_read(rotation2, scale, translation, fp2).is_none());
		}
	}
}
