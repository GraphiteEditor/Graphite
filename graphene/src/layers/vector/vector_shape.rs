use crate::layers::{
	id_storage::UniqueElements,
	layer_info::{Layer, LayerDataType},
};
use std::ops::{Deref, DerefMut};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, PathEl, Rect, Shape};
use serde::{Deserialize, Serialize};

/// VectorShape represents a single vector shape, containing many anchors
/// For each closed shape we keep a VectorShape which contains the handles and anchors that define that shape.
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VectorShape(UniqueElements<VectorAnchor>);

// TODO Implement iterator for VectorShape

impl VectorShape {
	pub fn new() -> Self {
		VectorShape { ..Default::default() }
	}

	// TODO Wrap this within an adapter to separate kurbo from VectorShape
	pub fn from_kurbo_shape<T: Shape>(shape: &T) -> Self {
		shape.path_elements(0.1).into()
	}

	pub fn anchors(&self) -> &UniqueElements<VectorAnchor> {
		&self.0
	}

	pub fn anchors_mut(&mut self) -> &mut UniqueElements<VectorAnchor> {
		&mut self.0
	}

	/// constructs a rectangle with `p1` as the lower left and `p2` as the top right
	pub fn new_rect(p1: DVec2, p2: DVec2) -> Self {
		VectorShape(
			vec![
				VectorAnchor::new(p1),
				VectorAnchor::new(DVec2::new(p1.x, p2.y)),
				VectorAnchor::new(p2),
				VectorAnchor::new(DVec2::new(p2.x, p1.y)),
				VectorAnchor::closed(),
			]
			.into_iter()
			.collect(),
		)
	}

	/// constructs an ngon
	/// `radius` is the distance from the center to any vertex, or the radius of the circle the ngon may be inscribed inside
	pub fn new_ngon(center: DVec2, sides: u64, radius: f64) -> Self {
		let mut anchors = vec![];
		for i in 0..sides {
			let angle = (i as f64) * std::f64::consts::TAU / (sides as f64);
			anchors.push(VectorAnchor::new(DVec2::new(center.x + radius * f64::cos(angle), center.y + radius * f64::sin(angle))));
		}
		anchors.push(VectorAnchor::closed());
		VectorShape(anchors.into_iter().collect())
	}

	/// constructs a line from `p1` to `p2`
	pub fn new_line(p1: DVec2, p2: DVec2) -> Self {
		VectorShape(vec![VectorAnchor::new(p1), VectorAnchor::new(p2)].into_iter().collect())
	}

	pub fn new_poly_line<T: Into<glam::DVec2>>(points: Vec<T>) -> Self {
		let anchors = points.into_iter().map(|point| VectorAnchor::new(point.into()));
		let mut p_line = VectorShape(UniqueElements::default());
		p_line.0.add_range(anchors, -1);
		p_line
	}

	pub fn move_selected(&mut self, delta: DVec2, relative: bool) {
		self.selected_anchors_mut().for_each(|anchor| anchor.move_selected_points(relative, &DAffine2::from_translation(delta)));
	}

	// TODO Implement deleting currently selected points
	pub fn delete_selected(&mut self) {
		// involves cloning the elements of anchors, could be replaced by a more efficient implementation possibly
		for (index, anchor) in self.selected_anchors_mut().enumerate() {
			// Example
			// anchor.delete_selected_points();
			// if anchor.points.is_empty() {
			// 	self.anchors.remove(index);
			// }
		}
	}

	// TODO Implement adding a point to a curve
	pub fn add_point(&mut self, nearest_point_on_curve: DVec2) {
		for anchor in self.selected_anchors_mut() {
			if anchor.is_anchor_selected() {
				// Example
				// anchor.add_point(anchor.control_points_mut(), nearest_point_on_curve);
			}
		}
	}

	/// Select an anchor by id
	pub fn select_anchor(&mut self, anchor_id: u64) -> Option<&mut VectorAnchor> {
		if let Some(anchor) = self.0.by_id_mut(anchor_id) {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// Select an anchor by index
	pub fn select_anchor_by_index(&mut self, anchor_index: usize) -> Option<&mut VectorAnchor> {
		// TODO test if looking this up by index actually works
		if let Some(anchor) = self.0.by_index_mut(anchor_index) {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// The last anchor in the shape thus far
	pub fn select_last_anchor(&mut self) -> Option<&mut VectorAnchor> {
		if let Some(anchor) = self.0.last_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// Deselect an anchor
	pub fn deselect_anchor(&mut self, anchor_id: u64) {
		if let Some(anchor) = self.0.by_id_mut(anchor_id) {
			anchor.clear_selected_points();
			anchor.select_point(ControlPointType::Anchor as usize, false);
		}
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self) {
		for anchor in self.0.iter_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
		}
	}

	/// Clear all the selected anchors, and clear the selected points on the anchors
	pub fn clear_selected_anchors(&mut self) {
		for anchor in self.0.iter_mut() {
			anchor.clear_selected_points();
		}
	}

	/// Return all the selected anchors by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.0.iter().filter(|anchor| anchor.is_anchor_selected())
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.0.iter_mut().filter(|anchor| anchor.is_anchor_selected())
	}

	// Kurbo removed from apply_affine
	pub fn apply_affine(&mut self, affine: DAffine2) {
		for anchor in self.0.iter_mut() {
			anchor.transform(&affine);
		}
	}

	// TODO Remove BezPath / kurbo reliance here
	pub fn bounding_box(&self) -> Rect {
		<&Self as Into<BezPath>>::into(self).bounding_box()
	}

	// TODO Abstract the usage of BezPath / Kurbo here
	pub fn to_svg(&mut self) -> String {
		<&Self as Into<BezPath>>::into(self).to_svg()
	}
}

/// Convert a mutable layer into a mutable VectorShape
impl<'a> TryFrom<&'a mut Layer> for &'a mut VectorShape {
	type Error = &'static str;
	fn try_from(layer: &'a mut Layer) -> Result<&'a mut VectorShape, Self::Error> {
		match &mut layer.data {
			LayerDataType::Shape(layer) => Ok(&mut layer.shape),
			// TODO Resolve converting text into a VectorShape at the layer level
			// LayerDataType::Text(text) => Some(VectorShape::new(path_to_shape.to_vec(), viewport_transform, true)),
			_ => Err("Did not find any shape data in the layer"),
		}
	}
}

/// Convert a reference to a layer into a reference of a VectorShape
impl<'a> TryFrom<&'a Layer> for &'a VectorShape {
	type Error = &'static str;
	fn try_from(layer: &'a Layer) -> Result<&'a VectorShape, Self::Error> {
		match &layer.data {
			LayerDataType::Shape(layer) => Ok(&layer.shape),
			// TODO Resolve converting text into a VectorShape at the layer level
			// LayerDataType::Text(text) => Some(VectorShape::new(path_to_shape.to_vec(), viewport_transform, true)),
			_ => Err("Did not find any shape data in the layer"),
		}
	}
}

/// Create a BezPath from a VectorShape
impl From<&VectorShape> for BezPath {
	fn from(vector_shape: &VectorShape) -> Self {
		if vector_shape.0.is_empty() {
			return BezPath::new();
		}

		let mut bez_path = vec![];
		let mut start_new_shape = true;

		for elements in vector_shape.0.windows(2) {
			let first = &elements[0];
			let second = &elements[1];

			if start_new_shape {
				if let Some(anchor) = &first.points[0] {
					bez_path.push(PathEl::MoveTo(point_to_kurbo(anchor)));
				}
				start_new_shape = false;
			}

			let new_segment = match [&first.points[2], &second.points[1], &second.points[0]] {
				[None, None, Some(p)] => PathEl::LineTo(point_to_kurbo(p)),
				[None, Some(a), Some(p)] => PathEl::QuadTo(point_to_kurbo(a), point_to_kurbo(p)),
				[Some(a1), Some(a2), Some(p)] => PathEl::CurveTo(point_to_kurbo(a1), point_to_kurbo(a2), point_to_kurbo(p)),
				[None, None, None] => {
					start_new_shape = true;
					PathEl::ClosePath
				}
				_ => panic!("Invalid path element"),
			};
			bez_path.push(new_segment);
		}
		BezPath::from_vec(bez_path)
	}
}

/// Create a VectorShape from a BezPath
impl<T: Iterator<Item = PathEl>> From<T> for VectorShape {
	fn from(path: T) -> Self {
		let mut vector_shape = VectorShape::new();
		for path_el in path {
			match path_el {
				PathEl::MoveTo(p) => {
					vector_shape.0.add(VectorAnchor::new(kurbo_point_to_dvec2(p)), None, -1);
				}
				PathEl::LineTo(p) => {
					vector_shape.0.add(VectorAnchor::new(kurbo_point_to_dvec2(p)), None, -1);
				}
				PathEl::QuadTo(p0, p1) => {
					vector_shape.0.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::Handle2));
					vector_shape.0.add(VectorAnchor::new(kurbo_point_to_dvec2(p1)), None, -1);
					vector_shape.0.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::Handle1));
				}
				PathEl::CurveTo(p0, p1, p2) => {
					vector_shape.0.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::Handle2));
					vector_shape.0.add(VectorAnchor::new(kurbo_point_to_dvec2(p2)), None, -1);
					vector_shape.0.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p1), ControlPointType::Handle1));
				}
				PathEl::ClosePath => {
					vector_shape.0.add(VectorAnchor::closed(), None, -1);
				}
			}
		}
		vector_shape
	}
}

// allows access to anchors as slice or iterator
impl Deref for VectorShape {
	type Target = [VectorAnchor];
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

// allows mutable access to anchors as slice or iterator
impl DerefMut for VectorShape {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

///*Kurbo adaptors */
#[inline]
fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

#[inline]
fn point_to_kurbo(point: &VectorControlPoint) -> kurbo::Point {
	kurbo::Point::new(point.position.x, point.position.y)
}

#[inline]
fn kurbo_point_to_dvec2(point: kurbo::Point) -> DVec2 {
	DVec2::new(point.x, point.y)
}
