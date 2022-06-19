use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};
use crate::layers::{
	id_vec::IdBackedVec,
	layer_info::{Layer, LayerDataType},
};

use glam::{DAffine2, DVec2};
use kurbo::{BezPath, PathEl, Rect, Shape};
use serde::{Deserialize, Serialize};

/// VectorShape represents a single vector shape, containing many anchors
/// For each closed shape we keep a VectorShape which contains the handles and anchors that define that shape.
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VectorShape(IdBackedVec<VectorAnchor>);

impl VectorShape {
	// ** SHAPE INITIALIZATION **

	/// Create a new VectorShape with no anchors or handles
	pub fn new() -> Self {
		VectorShape { ..Default::default() }
	}

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

	pub fn new_ellipse(p1: DVec2, p2: DVec2) -> Self {
		let x_height = DVec2::new((p2.x - p1.x).abs(), 0.);
		let y_height = DVec2::new(0., (p2.y - p1.y).abs());
		let center = (p1 + p2) * 0.5;
		let top = center + y_height * 0.5;
		let bottom = center - y_height * 0.5;
		let left = center + x_height * 0.5;
		let right = center - x_height * 0.5;

		// Constant explained here https://stackoverflow.com/a/27863181
		let curve_constant = 0.55228_3;
		let handle_offset_x = x_height * curve_constant * 0.5;
		let handle_offset_y = y_height * curve_constant * 0.5;

		VectorShape(
			vec![
				VectorAnchor::new_with_handles(top, Some(top + handle_offset_x), Some(top - handle_offset_x)),
				VectorAnchor::new_with_handles(right, Some(right + handle_offset_y), Some(right - handle_offset_y)),
				VectorAnchor::new_with_handles(bottom, Some(bottom - handle_offset_x), Some(bottom + handle_offset_x)),
				VectorAnchor::new_with_handles(left, Some(left - handle_offset_y), Some(left + handle_offset_y)),
				VectorAnchor::closed(),
			]
			.into_iter()
			.collect(),
		)
	}

	/// constructs an ngon
	/// `radius` is the distance from the `center` to any vertex, or the radius of the circle the ngon may be inscribed inside
	/// `sides` is the number of sides
	pub fn new_ngon(center: DVec2, sides: u64, radius: f64) -> Self {
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
		let mut p_line = VectorShape(IdBackedVec::default());
		p_line.0.push_range(anchors);
		p_line
	}

	pub fn new_spline<T: Into<glam::DVec2>>(points: Vec<T>) -> Self {
		let mut new = Self::default();
		// shadow `points`
		let points: Vec<DVec2> = points.into_iter().map(Into::<glam::DVec2>::into).collect();

		// Number of points = number of points to find handles for
		let n = points.len();

		// matrix coefficients a, b and c (see https://mathworld.wolfram.com/CubicSpline.html)
		// because the 'a' coefficients are all 1 they need not be stored
		// this algorithm does a variation of the above algorithm.
		// Instead of using the traditional cubic: a + bt + ct^2 + dt^3, we use the bezier cubic.

		let mut b = vec![DVec2::new(4.0, 4.0); n];
		b[0] = DVec2::new(2.0, 2.0);
		b[n - 1] = DVec2::new(2.0, 2.0);

		let mut c = vec![DVec2::new(1.0, 1.0); n];

		// 'd' is the the second point in a cubic bezier, which is what we solve for
		let mut d = vec![DVec2::ZERO; n];

		d[0] = DVec2::new(2.0 * points[1].x + points[0].x, 2.0 * points[1].y + points[0].y);
		d[n - 1] = DVec2::new(3.0 * points[n - 1].x, 3.0 * points[n - 1].y);
		for idx in 1..(n - 1) {
			d[idx] = DVec2::new(4.0 * points[idx].x + 2.0 * points[idx + 1].x, 4.0 * points[idx].y + 2.0 * points[idx + 1].y);
		}

		// Solve with Thomas algorithm (see https://en.wikipedia.org/wiki/Tridiagonal_matrix_algorithm)
		// do row operations to eliminate `a` coefficients
		c[0] /= -b[0];
		d[0] /= -b[0];
		for i in 1..n {
			b[i] += c[i - 1];
			// for some reason the below line makes the borrow checker mad
			//d[i] += d[i-1]
			d[i] = d[i] + d[i - 1];
			c[i] /= -b[i];
			d[i] /= -b[i];
		}

		// at this point b[i] == -a[i + 1], a[i] == 0,
		// do row operations to eliminate 'c' coefficients and solve
		d[n - 1] *= -1.0;
		for i in (0..n - 1).rev() {
			d[i] = d[i] - (c[i] * d[i + 1]);
			d[i] *= -1.0; //d[i] /= b[i]
		}

		// given the second point in the n'th cubic bezier, the third point is given by 2 * points[n+1] - b[n+1].
		// to find 'handle1_pos' for the n'th point we need the n-1 cubic bezier
		new.0.push_end(VectorAnchor::new_with_handles(points[0], None, Some(d[0])));
		for i in 1..n - 1 {
			new.0.push_end(VectorAnchor::new_with_handles(points[i], Some(2.0 * points[i] - d[i]), Some(d[i])));
		}
		new.0.push_end(VectorAnchor::new_with_handles(points[n - 1], Some(2.0 * points[n - 1] - d[n - 1]), None));

		new
	}

	// TODO Implement add_point
	pub fn add_point_to_end(&mut self, anchor: VectorAnchor) {
		self.0.push_end(anchor);
	}

	/// Move the selected points by the delta vector
	pub fn move_selected(&mut self, delta: DVec2, absolute_position: DVec2, viewspace: &DAffine2) {
		self.selected_anchors_any_points_mut()
			.for_each(|anchor| anchor.move_selected_points(delta, absolute_position, viewspace));
	}

	/// Delete the selected points from the VectorShape
	pub fn delete_selected(&mut self) {
		let mut ids_to_delete: Vec<u64> = vec![];
		for (id, anchor) in self.anchors_mut().enumerate_mut() {
			if anchor.is_anchor_selected() {
				ids_to_delete.push(*id);
			} else {
				anchor.delete_selected();
			}
		}

		for id in ids_to_delete {
			self.anchors_mut().remove(id);
		}
	}

	// Apply a transformation to all of the VectorShape points
	pub fn apply_affine(&mut self, affine: DAffine2) {
		for anchor in self.anchors_mut().iter_mut() {
			anchor.transform(&affine);
		}
	}

	// ** SELECTION OF POINTS **

	/// Select a single point by providing (AnchorId, ControlPointType)
	pub fn select_point(&mut self, point: (u64, ControlPointType), selected: bool) -> Option<&mut VectorAnchor> {
		let (anchor_id, point_id) = point;
		if let Some(anchor) = self.anchors_mut().by_id_mut(anchor_id) {
			anchor.select_point(point_id as usize, selected);
			return Some(anchor);
		}
		None
	}

	/// Select points in the VectorShape, given by (AnchorId, ControlPointType)
	pub fn select_points(&mut self, points: &[(u64, ControlPointType)], selected: bool) {
		points.iter().for_each(|point| {
			self.select_point(*point, selected);
		});
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self) {
		for anchor in self.anchors_mut().iter_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
		}
	}

	/// Select an anchor by index
	pub fn select_anchor_by_index(&mut self, anchor_index: usize) -> Option<&mut VectorAnchor> {
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

	/// Return all the selected anchors, reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors().iter().filter(|anchor| anchor.is_anchor_selected())
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors_mut().iter_mut().filter(|anchor| anchor.is_anchor_selected())
	}

	/// Return all the selected anchors that have any children points selected, reference
	pub fn selected_anchors_any_points(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors().iter().filter(|anchor| anchor.any_points_selected())
	}

	/// Return all the selected anchors that have any children points selected, mutable
	pub fn selected_anchors_any_points_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors_mut().iter_mut().filter(|anchor| anchor.any_points_selected())
	}

	/// An alias for `self.0`
	pub fn anchors(&self) -> &IdBackedVec<VectorAnchor> {
		&self.0
	}

	/// An alias for `self.0` mutable
	pub fn anchors_mut(&mut self) -> &mut IdBackedVec<VectorAnchor> {
		&mut self.0
	}

	// ** INTERFACE WITH KURBO **

	// TODO Implement our own a local bounding box calculation
	pub fn bounding_box(&self) -> Rect {
		<&Self as Into<BezPath>>::into(self).bounding_box()
	}

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
		if vector_shape.anchors().is_empty() {
			return BezPath::new();
		}

		let mut bez_path = vec![];
		let mut start_new_shape = true;

		// Take anchors and create path elements, lines, quads or curves, or a close indicator
		let anchors_to_path_el = |first: &VectorAnchor, second: &VectorAnchor| -> (PathEl, bool) {
			match [
				&first.points[ControlPointType::OutHandle],
				&second.points[ControlPointType::InHandle],
				&second.points[ControlPointType::Anchor],
			] {
				[None, None, Some(anchor)] => (PathEl::LineTo(point_to_kurbo(anchor)), false),
				[None, Some(in_handle), Some(anchor)] => (PathEl::QuadTo(point_to_kurbo(in_handle), point_to_kurbo(anchor)), false),
				[Some(out_handle), None, Some(anchor)] => (PathEl::QuadTo(point_to_kurbo(out_handle), point_to_kurbo(anchor)), false),
				[Some(out_handle), Some(in_handle), Some(anchor)] => (PathEl::CurveTo(point_to_kurbo(out_handle), point_to_kurbo(in_handle), point_to_kurbo(anchor)), false),
				[Some(out_handle), None, None] => {
					if let Some(first_anchor) = vector_shape.anchors().first() {
						(
							if let Some(in_handle) = &first_anchor.points[ControlPointType::InHandle] {
								PathEl::CurveTo(
									point_to_kurbo(out_handle),
									point_to_kurbo(in_handle),
									point_to_kurbo(first_anchor.points[ControlPointType::Anchor].as_ref().unwrap()),
								)
							} else {
								PathEl::QuadTo(point_to_kurbo(out_handle), point_to_kurbo(first_anchor.points[ControlPointType::Anchor].as_ref().unwrap()))
							},
							true,
						)
					} else {
						(PathEl::ClosePath, true)
					}
				}
				[None, None, None] => (PathEl::ClosePath, true),
				_ => panic!("Invalid path element {:#?}", vector_shape),
			}
		};

		for elements in vector_shape.anchors().windows(2) {
			let first = &elements[0];
			let second = &elements[1];

			// Tell kurbo cursor to move to the first anchor
			if start_new_shape {
				if let Some(anchor) = &first.points[ControlPointType::Anchor] {
					bez_path.push(PathEl::MoveTo(point_to_kurbo(anchor)));
				}
				start_new_shape = false;
			}

			// Take anchors and create path elements, lines, quads or curves, or a close indicator
			let (path_el, should_start_new_shape) = anchors_to_path_el(first, second);
			start_new_shape = should_start_new_shape;
			bez_path.push(path_el);
		}

		// bez_path[1] = PathEl::CurveTo(, , point_to_kurbo(first.points))
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
					vector_shape.anchors_mut().push_end(VectorAnchor::new(kurbo_point_to_dvec2(p)));
				}
				PathEl::LineTo(p) => {
					vector_shape.anchors_mut().push_end(VectorAnchor::new(kurbo_point_to_dvec2(p)));
				}
				PathEl::QuadTo(p0, p1) => {
					vector_shape.anchors_mut().push_end(VectorAnchor::new(kurbo_point_to_dvec2(p1)));
					vector_shape.anchors_mut().last_mut().unwrap().points[ControlPointType::InHandle] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::InHandle));
				}
				PathEl::CurveTo(p0, p1, p2) => {
					vector_shape.anchors_mut().last_mut().unwrap().points[ControlPointType::OutHandle] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::OutHandle));
					vector_shape.anchors_mut().push_end(VectorAnchor::new(kurbo_point_to_dvec2(p2)));
					vector_shape.anchors_mut().last_mut().unwrap().points[ControlPointType::InHandle] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p1), ControlPointType::InHandle));
				}
				PathEl::ClosePath => {
					vector_shape.anchors_mut().push_end(VectorAnchor::closed());
				}
			}
		}
		vector_shape
	}
}

#[inline]
fn point_to_kurbo(point: &VectorControlPoint) -> kurbo::Point {
	kurbo::Point::new(point.position.x, point.position.y)
}

#[inline]
fn kurbo_point_to_dvec2(point: kurbo::Point) -> DVec2 {
	DVec2::new(point.x, point.y)
}
