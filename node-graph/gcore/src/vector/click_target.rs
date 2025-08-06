use crate::vector::PointId;
use crate::vector::misc::rect_with_size;
use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, DEFAULT_ACCURACY, PathSeg, Rect, Shape};

use super::algorithms::intersection::bezpath_and_segment_intersections;
use super::misc::{bezpath_loose_bounding_box, dvec2_to_point, is_bezpath_closed, pathseg_to_points, point_to_dvec2, rect_to_minmax, transform_rect};

#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ClickTargetType {
	BezPath(BezPath),
	FreePoint(FreePoint),
}

/// Represents a clickable target for the layer
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClickTarget {
	target_type: ClickTargetType,
	stroke_width: f64,
	bounding_box: Option<Rect>,
}

impl ClickTarget {
	pub fn new_with_bezpath(bezpath: BezPath, stroke_width: f64) -> Self {
		let bounding_box = bezpath_loose_bounding_box(&bezpath);
		Self {
			target_type: ClickTargetType::BezPath(bezpath),
			stroke_width,
			bounding_box,
		}
	}

	pub fn new_with_free_point(point: FreePoint) -> Self {
		const MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT: f64 = 1e-4 / 2.;
		let stroke_width = 10.;

		let bounding_box = Some(rect_with_size(point.position, MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT));

		Self {
			target_type: ClickTargetType::FreePoint(point),
			stroke_width,
			bounding_box,
		}
	}

	pub fn target_type(&self) -> &ClickTargetType {
		&self.target_type
	}

	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box.map(|bbox| rect_to_minmax(bbox))
	}

	pub fn bounding_box_center(&self) -> Option<DVec2> {
		self.bounding_box.map(|bbox| point_to_dvec2(bbox.center()))
	}

	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box.map(|bbox| {
			[
				transform.transform_point2(DVec2::new(bbox.min_x(), bbox.min_y())),
				transform.transform_point2(DVec2::new(bbox.max_x(), bbox.max_y())),
			]
		})
	}

	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		match self.target_type {
			ClickTargetType::BezPath(ref mut subpath) => {
				subpath.apply_affine(Affine::new(affine_transform.to_cols_array()));
			}
			ClickTargetType::FreePoint(ref mut point) => {
				point.apply_transform(affine_transform);
			}
		}
		self.update_bbox();
	}

	fn update_bbox(&mut self) {
		match self.target_type {
			ClickTargetType::BezPath(ref subpath) => {
				self.bounding_box = Some(subpath.bounding_box());
			}
			ClickTargetType::FreePoint(ref point) => {
				self.bounding_box = Some(rect_with_size(point.position, self.stroke_width));
			}
		}
	}

	/// Does the click target intersect the path
	pub fn intersect_path(&self, mut selection_bezpath: BezPath, layer_transform: DAffine2) -> bool {
		// Check if the matrix is not invertible
		let mut layer_transform = layer_transform;
		if layer_transform.matrix2.determinant().abs() <= f64::EPSILON {
			layer_transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		}

		let inverse = layer_transform.inverse();
		selection_bezpath.apply_affine(Affine::new(inverse.to_cols_array()));

		match self.target_type() {
			ClickTargetType::BezPath(click_target_bezpath) => {
				let inside = |segment: PathSeg| pathseg_to_points(segment).iter().filter_map(|point| *point).all(|point| selection_bezpath.contains(point));
				let intersects = |segment: PathSeg| !bezpath_and_segment_intersections(&selection_bezpath, segment, None, None).is_empty();

				click_target_bezpath.segments().any(|target_segment| inside(target_segment)) || click_target_bezpath.segments().any(|target_segment| intersects(target_segment))
			}
			ClickTargetType::FreePoint(point) => selection_bezpath.contains(dvec2_to_point(point.position)),
		}
	}

	/// Does the click target intersect the point (accounting for stroke size)
	pub fn intersect_point(&self, point: DVec2, layer_transform: DAffine2) -> bool {
		let target_bounds = rect_with_size(point, self.stroke_width);
		// This bounding box is not very accurate as it is the axis aligned version of the transformed bounding box. However it is fast.
		if !self.bounding_box.is_some_and(|bbox| (transform_rect(bbox, layer_transform)).overlaps(target_bounds)) {
			return false;
		}

		// Allows for selecting lines
		// TODO: actual intersection of stroke
		self.intersect_path(target_bounds.to_path(DEFAULT_ACCURACY), layer_transform)
	}

	/// Does the click target intersect the point (not accounting for stroke size)
	pub fn intersect_point_no_stroke(&self, point: DVec2) -> bool {
		// Check if the point is within the bounding box
		if self.bounding_box.is_some_and(|bbox| bbox.contains(dvec2_to_point(point))) {
			// Check if the point is within the shape
			match self.target_type() {
				ClickTargetType::BezPath(bezpath) => is_bezpath_closed(bezpath) && bezpath.contains(dvec2_to_point(point)),
				ClickTargetType::FreePoint(free_point) => free_point.position == point,
			}
		} else {
			false
		}
	}
}
