use std::sync::{Arc, RwLock};

use super::algorithms::{bezpath_algorithms::bezpath_is_inside_bezpath, intersection::filtered_segment_intersections};
use super::misc::dvec2_to_point;
use crate::math::math_ext::QuadExt;
use crate::math::quad::Quad;
use crate::subpath::Subpath;
use crate::transform::Transform;
use crate::vector::PointId;
use crate::vector::misc::point_to_dvec2;
use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, ParamCurve, PathSeg, Shape};

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
	Subpath(Subpath<PointId>),
	FreePoint(FreePoint),
}

#[derive(Clone, Debug, Default)]
struct BoundingBoxCache {
	fingerprints: u64,
	elements: [(f64, Option<[DVec2; 2]>); 8],
	write_ptr: usize,
}

/// Represents a clickable target for the layer
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClickTarget {
	target_type: ClickTargetType,
	stroke_width: f64,
	bounding_box: Option<[DVec2; 2]>,
	#[serde(skip)]
	bounding_box_cache: Arc<RwLock<BoundingBoxCache>>,
}

impl PartialEq for ClickTarget {
	fn eq(&self, other: &Self) -> bool {
		self.target_type == other.target_type && self.stroke_width == other.stroke_width && self.bounding_box == other.bounding_box
	}
}

impl ClickTarget {
	pub fn new_with_subpath(subpath: Subpath<PointId>, stroke_width: f64) -> Self {
		let bounding_box = subpath.loose_bounding_box();
		Self {
			target_type: ClickTargetType::Subpath(subpath),
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

	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box
	}

	pub fn bounding_box_center(&self) -> Option<DVec2> {
		self.bounding_box.map(|bbox| bbox[0] + (bbox[1] - bbox[0]) / 2.)
	}

	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		match self.target_type {
			ClickTargetType::Subpath(ref subpath) => {
				if transform.has_skew() {
					return subpath.bounding_box_with_transform(transform);
				}
				let rotation = transform.decompose_rotation();
				let fingerprint = (rotation.to_bits() % 265) as u8;

				let read = self.bounding_box_cache.read().unwrap();

				let mut mask: u8 = 0;
				// Split out mask construction into dedicated loop to allow for vectorization
				for (i, fp) in (0..8).zip(read.fingerprints.to_le_bytes()) {
					if fp == fingerprint {
						mask |= 1 << i;
					}
				}
				let scale = transform.decompose_scale();

				while mask != 0 {
					let pos = mask.trailing_zeros() as usize;

					if rotation == read.elements[pos].0 && read.elements[pos].1.is_some() {
						let transform = DAffine2::from_scale_angle_translation(scale, 0., transform.translation);
						let new_bounds = read.elements[pos].1.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)]);

						return new_bounds;
					}
					mask &= !(1 << pos);
				}

				std::mem::drop(read);
				let mut write = self.bounding_box_cache.write().unwrap();
				let bounds = subpath.bounding_box_with_transform(DAffine2::from_angle(rotation));

				if bounds.is_none() {
					return bounds;
				}

				let write_ptr = write.write_ptr;
				write.elements[write_ptr] = (rotation, bounds);
				let mut bytes = write.fingerprints.to_le_bytes();
				bytes[write_ptr] = fingerprint;
				write.fingerprints = u64::from_le_bytes(bytes);
				write.write_ptr = (write_ptr + 1) % write.elements.len();

				let transform = DAffine2::from_scale_angle_translation(scale, 0., transform.translation);
				bounds.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)])
			}
			// TODO: use point for calculation of bbox
			ClickTargetType::FreePoint(_) => self.bounding_box.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)]),
		}
	}

	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		match self.target_type {
			ClickTargetType::Subpath(ref mut subpath) => {
				subpath.apply_transform(affine_transform);
			}
			ClickTargetType::FreePoint(ref mut point) => {
				point.apply_transform(affine_transform);
			}
		}
		self.update_bbox();
	}

	fn update_bbox(&mut self) {
		match self.target_type {
			ClickTargetType::Subpath(ref subpath) => {
				self.bounding_box = subpath.bounding_box();
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
			ClickTargetType::Subpath(subpath) => {
				// Check if outlines intersect
				let outline_intersects = |path_segment: PathSeg| bezier_iter().any(|line| !filtered_segment_intersections(path_segment, line, None, None).is_empty());
				if subpath.iter().any(outline_intersects) {
					return true;
				}
				// Check if selection is entirely within the shape
				if subpath.closed() && bezier_iter().next().is_some_and(|bezier| subpath.contains_point(point_to_dvec2(bezier.start()))) {
					return true;
				}

				let mut selection = BezPath::from_path_segments(bezier_iter());
				selection.close_path();

				// Check if shape is entirely within selection
				bezpath_is_inside_bezpath(&subpath.to_bezpath(), &selection, None, None)
			}
			ClickTargetType::FreePoint(point) => bezier_iter().map(|bezier: PathSeg| bezier.winding(dvec2_to_point(point.position))).sum::<i32>() != 0,
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
				ClickTargetType::Subpath(subpath) => subpath.closed() && subpath.contains_point(point),
				ClickTargetType::FreePoint(free_point) => free_point.position == point,
			}
		} else {
			false
		}
	}
}
