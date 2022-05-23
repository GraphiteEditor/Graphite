use crate::layers::{
	vec_unique::VecUnique,
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
pub struct VectorShape(VecUnique<VectorAnchor>);

// TODO Implement iterator for VectorShape

impl VectorShape {
	// ** SHAPE INITIALIZATION **

	/// Create a new VectorShape with no anchors or handles
	pub fn new() -> Self {
		VectorShape { ..Default::default() }
	}

	// TODO Wrap this within an adapter to separate kurbo from VectorShape
	/// Create a new VectorShape from a kurbo Shape
	/// This exists to smooth the transition away from Kurbo
	pub fn from_kurbo_shape<T: Shape>(shape: &T) -> Self {
		shape.path_elements(0.1).into()
	}

	// ** PRIMITIVE CONSTRUCTION **

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

	/// Constructs an ngon
	/// `radius` is the distance from the center to any vertex, or the radius of the circle the ngon may be inscribed inside
	pub fn new_ngon(center: DVec2, sides: u32, radius: f64) -> Self {
		let mut anchors = vec![];
		for i in 0..sides {
			let angle = (i as f64) * std::f64::consts::TAU / (sides as f64);
			anchors.push(VectorAnchor::new(DVec2::new(center.x + radius * f64::cos(angle), center.y + radius * f64::sin(angle))));
		}
		anchors.push(VectorAnchor::closed());
		VectorShape(anchors.into_iter().collect())
	}

	/// Constructs a line from `p1` to `p2`
	pub fn new_line(p1: DVec2, p2: DVec2) -> Self {
		VectorShape(vec![VectorAnchor::new(p1), VectorAnchor::new(p2)].into_iter().collect())
	}

	/// Constructs a set of lines from `p1` to `pN`
	pub fn new_poly_line<T: Into<glam::DVec2>>(points: Vec<T>) -> Self {
		let anchors = points.into_iter().map(|point| VectorAnchor::new(point.into()));
		let mut p_line = VectorShape(VecUnique::default());
		p_line.0.push_range(anchors);
		p_line
	}

	// ** MANIPULATION OF POINTS **

	/// Add a new anchor at the closest position on the nearest curve
	pub fn add_point(&mut self, position_closest: DVec2) {
		// TODO Implement
	}

	/// Move the selected points by the delta vector
	pub fn move_selected(&mut self, position_delta: DVec2) {
		// TODO Implement
		// self.selected_anchors_mut().for_each(|anchor| anchor.move_selected_points(true, &DAffine2::from_translation(delta)));
	}

	/// Delete the selected points from the VectorShape
	pub fn delete_selected(&mut self) {
		let mut ids_to_delete: Vec<u64> = vec![];
		for (id, anchor) in self.anchors().enumerate() {
			if anchor.is_anchor_selected() {
				ids_to_delete.push(*id);
			}
		}

		for id in ids_to_delete {
			self.anchors_mut().remove(id);
		}
	}

	// Apply a transformation to all of the VectorShape points
	pub fn apply_affine(&mut self, affine: DAffine2) {
		for anchor in self.0.iter_mut() {
			anchor.transform(&affine);
		}
	}

	// ** SELECTION OF POINTS **

	/// Select an anchor by id
	pub fn select_anchor(&mut self, anchor_id: u64, selected: bool) -> Option<&mut VectorAnchor> {
		if let Some(anchor) = self.anchors_mut().by_id_mut(anchor_id) {
			anchor.select_point(ControlPointType::Anchor as usize, selected);
			return Some(anchor);
		}
		None
	}

	/// Select anchors by an array of IDs
	pub fn select_anchors(&mut self, anchor_ids: &[u64], selected: bool) {
		for anchor_id in anchor_ids {
			if let Some(anchor) = self.anchors_mut().by_id_mut(*anchor_id) {
				anchor.select_point(ControlPointType::Anchor as usize, selected);
			}
		}
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self) {
		for anchor in self.anchors_mut().iter_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
		}
	}

	
	/// Select an anchor by index
	pub fn select_anchor_by_index(&mut self, anchor_index: usize) -> Option<&mut VectorAnchor> {
		// TODO test if looking this up by index actually works
		if let Some(anchor) = self.anchors_mut().by_index_mut(anchor_index) {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}
	
	/// The last anchor in the shape
	pub fn select_last_anchor(&mut self) -> Option<&mut VectorAnchor> {
		if let Some(anchor) = self.anchors_mut().last_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// Clear all the selected anchors, and clear the selected points on the anchors
	pub fn clear_selected_anchors(&mut self) {
		for anchor in self.anchors_mut().iter_mut() {
			anchor.clear_selected_points();
		}
	}

	// ** ACCESSING ANCHORS **

	/// Return all the selected anchors by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors().iter().filter(|anchor| anchor.is_anchor_selected())
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors_mut().iter_mut().filter(|anchor| anchor.is_anchor_selected())
	}
	
	/// An alias for `self.0`
	pub fn anchors(&self) -> &VecUnique<VectorAnchor> {
		&self.0
	}

	/// An alias for `self.0` mutable
	pub fn anchors_mut(&mut self) -> &mut VecUnique<VectorAnchor> {
		&mut self.0
	}

	// ** INTERFACE WITH KURBO **
	
	// TODO Remove BezPath / kurbo reliance here
	pub fn bounding_box(&self) -> Rect {
		<&Self as Into<BezPath>>::into(self).bounding_box()
	}

	// TODO Abstract the usage of BezPath / Kurbo here
	pub fn to_svg(&mut self) -> String {
		<&Self as Into<BezPath>>::into(self).to_svg()
	}
}

// ** CONVERSIONS **

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
					vector_shape.0.push_end(VectorAnchor::new(kurbo_point_to_dvec2(p)));
				}
				PathEl::LineTo(p) => {
					vector_shape.0.push_end(VectorAnchor::new(kurbo_point_to_dvec2(p)));
				}
				PathEl::QuadTo(p0, p1) => {
					vector_shape.0.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::OutHandle));
					vector_shape.0.push_end(VectorAnchor::new(kurbo_point_to_dvec2(p1)));
					vector_shape.0.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::InHandle));
				}
				PathEl::CurveTo(p0, p1, p2) => {
					vector_shape.0.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::OutHandle));
					vector_shape.0.push_end(VectorAnchor::new(kurbo_point_to_dvec2(p2)));
					vector_shape.0.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p1), ControlPointType::InHandle));
				}
				PathEl::ClosePath => {
					vector_shape.0.push_end(VectorAnchor::closed());
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
