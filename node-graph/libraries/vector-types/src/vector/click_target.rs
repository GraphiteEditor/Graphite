use std::sync::{Arc, OnceLock, RwLock};

use super::algorithms::{bezpath_algorithms::bezpath_is_inside_bezpath, intersection::filtered_segment_intersections};
use super::misc::dvec2_to_point;
use crate::math::QuadExt;
use crate::vector::PointId;
use core_types::math::quad::Quad;
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

/// The control point hull of the path, a loose bound that is linear to compute, or None if the path is empty.
fn control_box_bounds(bezpath: &BezPath) -> BoundingBox {
	let control_box = bezpath.control_box();
	(!bezpath.elements().is_empty()).then(|| [DVec2::new(control_box.min_x(), control_box.min_y()), DVec2::new(control_box.max_x(), control_box.max_y())])
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
	/// One or more contours tested as one compound shape using the nonzero fill rule, so holes
	/// (e.g. the inside of an "O") correctly count as outside the fill.
	Path(BezPath),
	FreePoint(FreePoint),
}

/// Fixed-size ring buffer of tight bounding boxes of a path under normalized linear transforms.
///
/// Any invertible transform `T · L` is placed as `bounds(L · path) = s · bounds(L' · path) + t` with `L' = L / s` and `s = sqrt(|det L|)`,
/// so one entry serves every translation and uniform zoom of the same rotation, scale ratio, and skew.
#[derive(Clone, Debug, Default)]
struct BoundingBoxCache {
	/// The bit patterns of each entry's normalized linear part, and the path's bounds under it.
	entries: [Option<([u64; 4], [DVec2; 2])>; Self::CACHE_SIZE],
	/// Next position to write in the ring buffer.
	write_index: usize,
}

impl BoundingBoxCache {
	const CACHE_SIZE: usize = 8;

	fn read(&self, key: [u64; 4]) -> Option<[DVec2; 2]> {
		self.entries.iter().flatten().find(|(entry_key, _)| *entry_key == key).map(|(_, bounds)| *bounds)
	}

	fn write(&mut self, key: [u64; 4], bounds: [DVec2; 2]) {
		self.entries[self.write_index] = Some((key, bounds));
		self.write_index = (self.write_index + 1) % Self::CACHE_SIZE;
	}
}

/// A path target's geometry in its own space, shared by every target (in any frame) drawing the same shape.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "BezPath", into = "BezPath"))]
pub struct TargetPath {
	path: BezPath,
	control_bounds: BoundingBox,
	closed_contours: OnceLock<BezPath>,
	bounding_box_cache: RwLock<BoundingBoxCache>,
}

impl TargetPath {
	pub fn new(path: BezPath) -> Self {
		Self {
			control_bounds: control_box_bounds(&path),
			path,
			closed_contours: OnceLock::new(),
			bounding_box_cache: Default::default(),
		}
	}

	/// The contours combined into one compound path, closing each open one when `close_open_contours` is set, or None if all are empty.
	pub fn from_contours(contours: &[BezPath], close_open_contours: bool) -> Option<Self> {
		let nonempty_contours = || contours.iter().filter(|contour| !contour.elements().is_empty());
		nonempty_contours().next()?;

		let mut elements = Vec::with_capacity(nonempty_contours().map(|contour| contour.elements().len() + 1).sum());
		for contour in nonempty_contours() {
			elements.extend_from_slice(contour.elements());
			if close_open_contours && !matches!(contour.elements().last(), Some(PathEl::ClosePath)) {
				elements.push(PathEl::ClosePath);
			}
		}

		Some(Self::new(BezPath::from_vec(elements)))
	}

	/// The contours as outlines plus the faces as the fill region, for a mesh whose fill paints its faces rather than its contours, or None if all are empty.
	/// No contour stays closed, so none adds fill that could cancel a face's under the nonzero rule.
	pub fn from_contours_and_faces(contours: &[BezPath], faces: &[BezPath]) -> Option<Self> {
		let mut elements = Vec::new();

		// A closed contour draws out its closing edge instead, keeping its whole outline without enclosing anything
		for contour in contours {
			let mut subpath_start = None;
			let mut previous_end = None;
			for &element in contour.elements() {
				match element {
					PathEl::ClosePath => {
						if let Some(start) = subpath_start
							&& previous_end != Some(start)
						{
							elements.push(PathEl::LineTo(start));
						}
						previous_end = subpath_start;
					}
					_ => {
						if let PathEl::MoveTo(point) = element {
							subpath_start = Some(point);
						}
						previous_end = element.end_point();
						elements.push(element);
					}
				}
			}
		}

		// Each loop of a face is closed so it counts toward the fill region
		for face in faces {
			let mut loop_open = false;
			for &element in face.elements() {
				if loop_open && matches!(element, PathEl::MoveTo(_)) {
					elements.push(PathEl::ClosePath);
				}
				loop_open = !matches!(element, PathEl::ClosePath);
				elements.push(element);
			}
			if loop_open {
				elements.push(PathEl::ClosePath);
			}
		}

		(!elements.is_empty()).then(|| Self::new(BezPath::from_vec(elements)))
	}

	pub fn path(&self) -> &BezPath {
		&self.path
	}

	fn closed_contours(&self) -> &BezPath {
		self.closed_contours.get_or_init(|| closed_contours(&self.path))
	}

	/// Tight bounds of the path placed by `transform`, cached per normalized linear part (see [`BoundingBoxCache`]).
	fn bounding_box_with_transform(&self, transform: DAffine2) -> BoundingBox {
		let determinant = transform.matrix2.determinant();
		if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
			return bezpath_bounding_box_with_transform(&self.path, transform);
		}

		let scale = determinant.abs().sqrt();
		let normalized = transform.matrix2 * (1. / scale);
		// Rounding off the last mantissa bits lets zooms whose normalization differs by a few ulps share an entry
		const IGNORED_MANTISSA_BITS: u64 = 0xFFFF;
		let key = normalized.to_cols_array().map(|value| value.to_bits() & !IGNORED_MANTISSA_BITS);
		let place = |[min, max]: [DVec2; 2]| [min * scale + transform.translation, max * scale + transform.translation];

		if let Some(bounds) = self.bounding_box_cache.read().ok().and_then(|cache| cache.read(key)) {
			return Some(place(bounds));
		}

		let bounds = bezpath_bounding_box_with_transform(&self.path, DAffine2::from_mat2(normalized))?;
		if let Ok(mut cache) = self.bounding_box_cache.write() {
			cache.write(key, bounds);
		}

		Some(place(bounds))
	}
}

impl Clone for TargetPath {
	fn clone(&self) -> Self {
		Self::new(self.path.clone())
	}
}

impl PartialEq for TargetPath {
	fn eq(&self, other: &Self) -> bool {
		self.path == other.path
	}
}

impl From<BezPath> for TargetPath {
	fn from(path: BezPath) -> Self {
		Self::new(path)
	}
}

impl From<TargetPath> for BezPath {
	fn from(target_path: TargetPath) -> Self {
		target_path.path
	}
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum TargetShape {
	/// Shared geometry placed into the target's space by `transform`, so moving a target never copies its path.
	Path {
		geometry: Arc<TargetPath>,
		transform: DAffine2,
	},
	FreePoint(FreePoint),
}

impl PartialEq for TargetShape {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(
				Self::Path { geometry, transform },
				Self::Path {
					geometry: other_geometry,
					transform: other_transform,
				},
			) => transform == other_transform && (Arc::ptr_eq(geometry, other_geometry) || geometry == other_geometry),
			(Self::FreePoint(point), Self::FreePoint(other_point)) => point == other_point,
			_ => false,
		}
	}
}

/// Represents a clickable target for the layer
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClickTarget {
	shape: TargetShape,
	stroke_width: f64,
	bounding_box: BoundingBox,
}

impl ClickTarget {
	pub fn new_with_path(path: BezPath, stroke_width: f64) -> Self {
		Self::new_with_shared_path(Arc::new(TargetPath::new(path)), DAffine2::IDENTITY, stroke_width)
	}

	/// A target drawing shared geometry, placed into the target's space by `transform`.
	pub fn new_with_shared_path(geometry: Arc<TargetPath>, transform: DAffine2, stroke_width: f64) -> Self {
		let bounding_box = Self::path_loose_bounds(&geometry, transform);

		Self {
			shape: TargetShape::Path { geometry, transform },
			stroke_width,
			bounding_box,
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
			shape: TargetShape::FreePoint(point),
			stroke_width,
			bounding_box,
		}
	}

	/// The target's shape in its own space, copying and transforming a path target's geometry.
	pub fn target_type(&self) -> ClickTargetType {
		match &self.shape {
			TargetShape::Path { geometry, transform } => ClickTargetType::Path(Affine::new(transform.to_cols_array()) * geometry.path()),
			TargetShape::FreePoint(point) => ClickTargetType::FreePoint(*point),
		}
	}

	/// A path target's untransformed geometry and the transform placing it into the target's space.
	pub fn path_with_transform(&self) -> Option<(&BezPath, DAffine2)> {
		match &self.shape {
			TargetShape::Path { geometry, transform } => Some((geometry.path(), *transform)),
			TargetShape::FreePoint(_) => None,
		}
	}

	/// Loose bounds (the transformed control point hull) in the target's own space. Use [`Self::bounding_box_with_transform`] for tight bounds.
	pub fn bounding_box(&self) -> BoundingBox {
		self.bounding_box
	}

	pub fn bounding_box_center(&self) -> Option<DVec2> {
		self.bounding_box.map(|bbox| bbox[0] + (bbox[1] - bbox[0]) / 2.)
	}

	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> BoundingBox {
		match &self.shape {
			TargetShape::Path { geometry, transform: placement } => geometry.bounding_box_with_transform(transform * *placement),
			// TODO: use point for calculation of bbox
			TargetShape::FreePoint(_) => self.bounding_box.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)]),
		}
	}

	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		match &mut self.shape {
			TargetShape::Path { geometry, transform } => {
				*transform = affine_transform * *transform;
				self.bounding_box = Self::path_loose_bounds(geometry, *transform);
			}
			TargetShape::FreePoint(point) => {
				point.apply_transform(affine_transform);
				self.bounding_box = Some([point.position - DVec2::splat(self.stroke_width / 2.), point.position + DVec2::splat(self.stroke_width / 2.)]);
			}
		}
	}

	fn path_loose_bounds(geometry: &TargetPath, transform: DAffine2) -> BoundingBox {
		let bounds = geometry.control_bounds?;
		if transform == DAffine2::IDENTITY {
			return Some(bounds);
		}

		Some((transform * Quad::from_box(bounds)).bounding_box())
	}

	/// Does the click target intersect the path
	pub fn intersect_path<It: Iterator<Item = PathSeg>>(&self, mut bezier_iter: impl FnMut() -> It, layer_transform: DAffine2) -> bool {
		// A collapsed placement has no inverse, so its placed copy is tested under the layer transform alone
		if let TargetShape::Path { geometry, transform } = &self.shape
			&& transform.matrix2.determinant().abs() <= f64::EPSILON
		{
			let placed = Self::new_with_path(Affine::new(transform.to_cols_array()) * geometry.path(), self.stroke_width);
			return placed.intersect_path(bezier_iter, layer_transform);
		}

		// The query maps into the geometry's own space, so shared geometry is tested without being transformed
		let mut layer_transform = match &self.shape {
			TargetShape::Path { transform, .. } => layer_transform * *transform,
			TargetShape::FreePoint(_) => layer_transform,
		};

		// Check if the matrix is not invertible
		if layer_transform.matrix2.determinant().abs() <= f64::EPSILON {
			layer_transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		}

		let inverse = layer_transform.inverse();
		let mut bezier_iter = || bezier_iter().map(|bezier| Affine::new(inverse.to_cols_array()) * bezier);

		match &self.shape {
			TargetShape::Path { geometry, .. } => {
				let path = geometry.path();

				// Outline intersection (catches strokes and both filled/unfilled shapes)
				let outline_intersects = |path_segment: PathSeg| bezier_iter().any(|line| !filtered_segment_intersections(path_segment, line, None, None).is_empty());
				if path.segments().any(outline_intersects) {
					return true;
				}

				// Selection point inside the fill (nonzero rule).
				// Only closed contours contribute to the fill region; open segments would otherwise produce spurious winding on one side of the segment.
				let fill_region = geometry.closed_contours();
				if !fill_region.is_empty() && bezier_iter().next().is_some_and(|segment| fill_region.contains(segment.start())) {
					return true;
				}

				// Build closed selection path, then check if the whole shape is entirely within it
				let mut selection = BezPath::from_path_segments(bezier_iter());
				selection.close_path();
				bezpath_is_inside_bezpath(path, &selection, None, None)
			}
			TargetShape::FreePoint(point) => bezier_iter().map(|segment: PathSeg| segment.winding(dvec2_to_point(point.position))).sum::<i32>() != 0,
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
		if !self
			.bounding_box
			.is_some_and(|bbox| bbox[0].x <= point.x && point.x <= bbox[1].x && bbox[0].y <= point.y && point.y <= bbox[1].y)
		{
			return false;
		}

		// Check if the point is within the shape
		match &self.shape {
			TargetShape::Path { geometry, transform } => {
				if transform.matrix2.determinant() == 0. {
					return false;
				}
				let local_point = transform.inverse().transform_point2(point);
				geometry.closed_contours().contains(dvec2_to_point(local_point))
			}
			TargetShape::FreePoint(free_point) => free_point.position == point,
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

	fn assert_bounds_eq(actual: BoundingBox, expected: BoundingBox) {
		let (Some(actual), Some(expected)) = (actual, expected) else {
			assert_eq!(actual, expected);
			return;
		};
		assert!(actual[0].abs_diff_eq(expected[0], 1e-9) && actual[1].abs_diff_eq(expected[1], 1e-9), "{actual:?} != {expected:?}");
	}

	fn cached_entries(target: &ClickTarget) -> usize {
		let TargetShape::Path { geometry, .. } = &target.shape else { return 0 };
		geometry.bounding_box_cache.read().unwrap().entries.iter().flatten().count()
	}

	#[test]
	fn cached_bounds_match_the_exact_bounds() {
		let path = rectangle_path(DVec2::ZERO, DVec2::new(100., 50.));
		let click_target = ClickTarget::new_with_path(path.clone(), 1.);

		let transforms = [
			DAffine2::from_scale_angle_translation(DVec2::splat(1.5), PI / 6., DVec2::new(20., 30.)),
			DAffine2::from_scale_angle_translation(DVec2::new(2., 1.), PI / 4., DVec2::new(-5., 7.)),
			DAffine2::from_scale_angle_translation(DVec2::new(-1., 3.), 1., DVec2::ZERO),
			DAffine2::from_cols_array(&[2., 0.5, 0., 1., 10., 20.]),
		];
		for transform in transforms {
			// Once computing the entry and once reading it back
			assert_bounds_eq(click_target.bounding_box_with_transform(transform), bezpath_bounding_box_with_transform(&path, transform));
			assert_bounds_eq(click_target.bounding_box_with_transform(transform), bezpath_bounding_box_with_transform(&path, transform));
		}
	}

	#[test]
	fn panning_and_zooming_reuse_one_cache_entry() {
		let click_target = ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, DVec2::new(100., 50.)), 1.);
		let rotation = PI / 6.;

		for (zoom, pan) in [(1., DVec2::ZERO), (1.1, DVec2::new(50., 60.)), (0.37, DVec2::new(-3., 9.)), (7.3, DVec2::ONE)] {
			click_target.bounding_box_with_transform(DAffine2::from_scale_angle_translation(DVec2::new(2., 1.) * zoom, rotation, pan));
		}

		assert_eq!(cached_entries(&click_target), 1);
	}

	#[test]
	fn the_cache_keeps_the_most_recent_entries() {
		let click_target = ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, DVec2::splat(10.)), 1.);

		for index in 0..10 {
			click_target.bounding_box_with_transform(DAffine2::from_angle(index as f64 * PI / 16.));
		}

		assert_eq!(cached_entries(&click_target), BoundingBoxCache::CACHE_SIZE);
	}

	#[test]
	fn degenerate_transforms_bypass_the_cache() {
		let path = rectangle_path(DVec2::ZERO, DVec2::splat(10.));
		let click_target = ClickTarget::new_with_path(path.clone(), 1.);
		let flattening = DAffine2::from_scale(DVec2::new(1., 0.));

		assert_bounds_eq(click_target.bounding_box_with_transform(flattening), bezpath_bounding_box_with_transform(&path, flattening));
		assert_eq!(cached_entries(&click_target), 0);
	}

	#[test]
	fn a_placed_shared_path_hits_like_a_baked_copy() {
		let path = rectangle_path(DVec2::ZERO, DVec2::new(10., 5.));
		let placement = DAffine2::from_scale_angle_translation(DVec2::new(3., 1.), 0.3, DVec2::new(40., -20.));

		let shared = ClickTarget::new_with_shared_path(Arc::new(TargetPath::new(path.clone())), placement, 2.);
		let baked = ClickTarget::new_with_path(Affine::new(placement.to_cols_array()) * &path, 2.);

		let layer_transform = DAffine2::from_scale_angle_translation(DVec2::splat(0.5), -0.2, DVec2::new(3., 4.));
		for x in (-10..60).step_by(3) {
			for y in (-30..30).step_by(3) {
				let point = DVec2::new(x as f64, y as f64);
				assert_eq!(shared.intersect_point(point, layer_transform), baked.intersect_point(point, layer_transform), "at {point}");
				assert_eq!(shared.intersect_point_no_stroke(point), baked.intersect_point_no_stroke(point), "at {point}");
			}
		}
	}

	#[test]
	fn a_mesh_target_fills_its_faces_but_not_its_open_contours() {
		// An open corner whose closing would enclose a triangle, beside an unclosed square face like the ones meshes construct
		let mut corner = BezPath::new();
		corner.move_to((0., 0.));
		corner.line_to((0., 10.));
		corner.line_to((10., 10.));
		let mut face = BezPath::new();
		face.move_to((20., 0.));
		face.line_to((30., 0.));
		face.line_to((30., 10.));
		face.line_to((20., 10.));
		face.line_to((20., 0.));

		let target = ClickTarget::new_with_shared_path(Arc::new(TargetPath::from_contours_and_faces(&[corner], &[face]).unwrap()), DAffine2::IDENTITY, 2.);

		assert!(target.intersect_point_no_stroke(DVec2::new(25., 5.)));
		assert!(!target.intersect_point_no_stroke(DVec2::new(3., 7.)));
		assert!(target.intersect_point(DVec2::new(0., 5.), DAffine2::IDENTITY));
	}

	#[test]
	fn a_closed_contour_wound_against_its_face_keeps_the_face_filled() {
		let contour = rectangle_path(DVec2::ZERO, DVec2::splat(10.));
		let face = contour.reverse_subpaths();

		let target = ClickTarget::new_with_shared_path(Arc::new(TargetPath::from_contours_and_faces(&[contour], &[face]).unwrap()), DAffine2::IDENTITY, 0.);

		assert!(target.intersect_point_no_stroke(DVec2::splat(5.)));
	}

	#[test]
	fn moving_a_target_keeps_its_geometry_shared() {
		let mut target = ClickTarget::new_with_path(rectangle_path(DVec2::ZERO, DVec2::ONE), 0.);
		let original = target.clone();

		target.apply_transform(DAffine2::from_translation(DVec2::new(5., 0.)));

		let (TargetShape::Path { geometry, .. }, TargetShape::Path { geometry: original_geometry, .. }) = (&target.shape, &original.shape) else {
			panic!("Both targets are paths");
		};
		assert!(Arc::ptr_eq(geometry, original_geometry));
		assert_eq!(target.bounding_box(), Some([DVec2::new(5., 0.), DVec2::new(6., 1.)]));
	}
}
