use super::consts::ManipulatorType;
use super::manipulator_group::ManipulatorGroup;
use super::manipulator_point::ManipulatorPoint;
use crate::layers::id_vec::IdBackedVec;
use crate::layers::layer_info::{Layer, LayerDataType};

use glam::{DAffine2, DVec2};
use kurbo::{BezPath, PathEl, Rect, Shape};
use serde::{Deserialize, Serialize};

/// [Subpath] represents a single vector path, containing many [ManipulatorGroups].
/// For each closed shape we keep a [Subpath] which contains the [ManipulatorGroup]s (handles and anchors) that define that shape.
// TODO Add "closed" bool to subpath
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Subpath(IdBackedVec<ManipulatorGroup>);

impl Subpath {
	// ** INITIALIZATION **

	/// Create a new [Subpath] with no [ManipulatorGroup]s.
	pub fn new() -> Self {
		Subpath { ..Default::default() }
	}

	/// Construct a [Subpath] from a point iterator
	pub fn from_points(points: impl Iterator<Item = DVec2>, closed: bool) -> Self {
		let manipulator_groups = points.map(ManipulatorGroup::new_with_anchor);

		let mut p_line = Subpath(IdBackedVec::default());

		p_line.0.push_range(manipulator_groups);
		if closed {
			p_line.0.push(ManipulatorGroup::closed());
		}

		p_line
	}

	/// Create a new [Subpath] from a [kurbo Shape](Shape).
	/// This exists to smooth the transition away from Kurbo
	pub fn from_kurbo_shape<T: Shape>(shape: &T) -> Self {
		shape.path_elements(0.1).into()
	}

	// ** PRIMITIVE CONSTRUCTION **

	/// constructs a rectangle with `p1` as the lower left and `p2` as the top right
	pub fn new_rect(p1: DVec2, p2: DVec2) -> Self {
		Subpath(
			vec![
				ManipulatorGroup::new_with_anchor(p1),
				ManipulatorGroup::new_with_anchor(DVec2::new(p1.x, p2.y)),
				ManipulatorGroup::new_with_anchor(p2),
				ManipulatorGroup::new_with_anchor(DVec2::new(p2.x, p1.y)),
				ManipulatorGroup::closed(),
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

		Subpath(
			vec![
				ManipulatorGroup::new_with_handles(top, Some(top + handle_offset_x), Some(top - handle_offset_x)),
				ManipulatorGroup::new_with_handles(right, Some(right + handle_offset_y), Some(right - handle_offset_y)),
				ManipulatorGroup::new_with_handles(bottom, Some(bottom - handle_offset_x), Some(bottom + handle_offset_x)),
				ManipulatorGroup::new_with_handles(left, Some(left - handle_offset_y), Some(left + handle_offset_y)),
				ManipulatorGroup::closed(),
			]
			.into_iter()
			.collect(),
		)
	}

	/// constructs an ngon
	/// `radius` is the distance from the `center` to any vertex, or the radius of the circle the ngon may be inscribed inside
	/// `sides` is the number of sides
	pub fn new_ngon(center: DVec2, sides: u64, radius: f64) -> Self {
		let mut manipulator_groups = vec![];
		for i in 0..sides {
			let angle = (i as f64) * std::f64::consts::TAU / (sides as f64);
			let center = center + DVec2::ONE * radius;
			let position = ManipulatorGroup::new_with_anchor(DVec2::new(center.x + radius * f64::cos(angle), center.y + radius * f64::sin(angle)) * 0.5);

			manipulator_groups.push(position);
		}
		manipulator_groups.push(ManipulatorGroup::closed());

		Subpath(manipulator_groups.into_iter().collect())
	}

	/// Constructs a line from `p1` to `p2`
	pub fn new_line(p1: DVec2, p2: DVec2) -> Self {
		Subpath(vec![ManipulatorGroup::new_with_anchor(p1), ManipulatorGroup::new_with_anchor(p2)].into_iter().collect())
	}

	/// Constructs a set of lines from `p1` to `pN`
	pub fn new_poly_line<T: Into<glam::DVec2>>(points: Vec<T>) -> Self {
		let manipulator_groups = points.into_iter().map(|point| ManipulatorGroup::new_with_anchor(point.into()));
		let mut p_line = Subpath(IdBackedVec::default());
		p_line.0.push_range(manipulator_groups);
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
		new.0.push_end(ManipulatorGroup::new_with_handles(points[0], None, Some(d[0])));
		for i in 1..n - 1 {
			new.0.push_end(ManipulatorGroup::new_with_handles(points[i], Some(2.0 * points[i] - d[i]), Some(d[i])));
		}
		new.0.push_end(ManipulatorGroup::new_with_handles(points[n - 1], Some(2.0 * points[n - 1] - d[n - 1]), None));

		new
	}

	/// Move the selected points by the delta vector
	pub fn move_selected(&mut self, delta: DVec2, absolute_position: DVec2, viewspace: &DAffine2) {
		self.selected_manipulator_groups_any_points_mut()
			.for_each(|manipulator_group| manipulator_group.move_selected_points(delta, absolute_position, viewspace));
	}

	/// Delete the selected points from the [Subpath]
	pub fn delete_selected(&mut self) {
		let mut ids_to_delete: Vec<u64> = vec![];
		for (id, manipulator_group) in self.manipulator_groups_mut().enumerate_mut() {
			if manipulator_group.is_anchor_selected() {
				ids_to_delete.push(*id);
			} else {
				manipulator_group.delete_selected();
			}
		}

		for id in ids_to_delete {
			self.manipulator_groups_mut().remove(id);
		}
	}

	// Apply a transformation to all of the Subpath points
	pub fn apply_affine(&mut self, affine: DAffine2) {
		for manipulator_group in self.manipulator_groups_mut().iter_mut() {
			manipulator_group.transform(&affine);
		}
	}

	// ** SELECTION OF POINTS **

	/// Set a single point to a chosen selection state by providing `(manipulator group ID, manipulator type)`.
	pub fn select_point(&mut self, point: (u64, ManipulatorType), selected: bool) -> Option<&mut ManipulatorGroup> {
		let (manipulator_group_id, point_id) = point;
		if let Some(manipulator_group) = self.manipulator_groups_mut().by_id_mut(manipulator_group_id) {
			manipulator_group.select_point(point_id as usize, selected);

			Some(manipulator_group)
		} else {
			None
		}
	}

	/// Set points in the [Subpath] to a chosen selection state, given by `(manipulator group ID, manipulator type)`.
	pub fn select_points(&mut self, points: &[(u64, ManipulatorType)], selected: bool) {
		points.iter().for_each(|point| {
			self.select_point(*point, selected);
		});
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self) {
		for manipulator_group in self.manipulator_groups_mut().iter_mut() {
			manipulator_group.select_point(ManipulatorType::Anchor as usize, true);
		}
	}

	/// Select an anchor by index
	pub fn select_anchor_by_index(&mut self, manipulator_group_index: usize) -> Option<&mut ManipulatorGroup> {
		if let Some(manipulator_group) = self.manipulator_groups_mut().by_index_mut(manipulator_group_index) {
			manipulator_group.select_point(ManipulatorType::Anchor as usize, true);

			Some(manipulator_group)
		} else {
			None
		}
	}

	/// The last anchor in the shape
	pub fn select_last_anchor(&mut self) -> Option<&mut ManipulatorGroup> {
		if let Some(manipulator_group) = self.manipulator_groups_mut().last_mut() {
			manipulator_group.select_point(ManipulatorType::Anchor as usize, true);

			Some(manipulator_group)
		} else {
			None
		}
	}

	/// Clear all the selected manipulator groups, i.e., clear the selected points inside the manipulator groups
	pub fn clear_selected_manipulator_groups(&mut self) {
		for manipulator_group in self.manipulator_groups_mut().iter_mut() {
			manipulator_group.clear_selected_points();
		}
	}

	// ** ACCESSING MANIPULATORGROUPS **

	/// Return all the selected anchors, reference
	pub fn selected_manipulator_groups(&self) -> impl Iterator<Item = &ManipulatorGroup> {
		self.manipulator_groups().iter().filter(|manipulator_group| manipulator_group.is_anchor_selected())
	}

	/// Return all the selected anchors, mutable
	pub fn selected_manipulator_groups_mut(&mut self) -> impl Iterator<Item = &mut ManipulatorGroup> {
		self.manipulator_groups_mut().iter_mut().filter(|manipulator_group| manipulator_group.is_anchor_selected())
	}

	/// Return all the selected [ManipulatorPoint]s by reference
	pub fn selected_manipulator_groups_any_points(&self) -> impl Iterator<Item = &ManipulatorGroup> {
		self.manipulator_groups().iter().filter(|manipulator_group| manipulator_group.any_points_selected())
	}

	/// Return all the selected [ManipulatorPoint]s by mutable reference
	pub fn selected_manipulator_groups_any_points_mut(&mut self) -> impl Iterator<Item = &mut ManipulatorGroup> {
		self.manipulator_groups_mut().iter_mut().filter(|manipulator_group| manipulator_group.any_points_selected())
	}

	/// An alias for `self.0`
	pub fn manipulator_groups(&self) -> &IdBackedVec<ManipulatorGroup> {
		&self.0
	}

	/// Returns a [ManipulatorPoint] from the last [ManipulatorGroup] in the [Subpath].
	pub fn last_point(&self, control_type: ManipulatorType) -> Option<&ManipulatorPoint> {
		self.manipulator_groups().last().and_then(|manipulator_group| manipulator_group.points[control_type].as_ref())
	}

	/// Returns a [ManipulatorPoint] from the last [ManipulatorGroup], mutably
	pub fn last_point_mut(&mut self, control_type: ManipulatorType) -> Option<&mut ManipulatorPoint> {
		self.manipulator_groups_mut().last_mut().and_then(|manipulator_group| manipulator_group.points[control_type].as_mut())
	}

	/// Returns a [ManipulatorPoint]  from the first [ManipulatorGroup]
	pub fn first_point(&self, control_type: ManipulatorType) -> Option<&ManipulatorPoint> {
		self.manipulator_groups().first().and_then(|manipulator_group| manipulator_group.points[control_type].as_ref())
	}

	/// Returns a [ManipulatorPoint] from the first [ManipulatorGroup]
	pub fn first_point_mut(&mut self, control_type: ManipulatorType) -> Option<&mut ManipulatorPoint> {
		self.manipulator_groups_mut().first_mut().and_then(|manipulator_group| manipulator_group.points[control_type].as_mut())
	}

	/// Should we close the shape?
	pub fn should_close_shape(&self) -> bool {
		if self.last_point(ManipulatorType::Anchor).is_none() {
			return false;
		}

		self.first_point(ManipulatorType::Anchor)
			.unwrap()
			.position
			.distance(self.last_point(ManipulatorType::Anchor).unwrap().position)
			< 0.001 // TODO Replace with constant, a small epsilon
	}

	/// Close the shape if able
	pub fn close_shape(&mut self) {
		if self.should_close_shape() {
			self.manipulator_groups_mut().push_end(ManipulatorGroup::closed());
		}
	}

	/// An alias for `self.0` mutable
	pub fn manipulator_groups_mut(&mut self) -> &mut IdBackedVec<ManipulatorGroup> {
		&mut self.0
	}

	/// Return the bounding box of the shape
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bezier_iter()
			.map(|bezier| bezier.internal.bounding_box())
			.reduce(|[a_min, a_max], [b_min, b_max]| [a_min.min(b_min), a_max.max(b_max)])
	}

	/// Convert path to svg
	pub fn to_svg(&mut self) -> String {
		fn write_positions(result: &mut String, values: [Option<DVec2>; 3]) {
			use std::fmt::Write;
			let count = values.into_iter().flatten().count();
			for (index, pos) in values.into_iter().flatten().enumerate() {
				write!(result, "{},{}", pos.x, pos.y).unwrap();
				if index != count - 1 {
					result.push(' ');
				}
			}
		}

		let mut result = String::new();
		// The out position from the previous ManipulatorGroup
		let mut last_out_handle = None;
		// The values from the last moveto (for closing the path)
		let (mut first_in_handle, mut first_in_anchor) = (None, None);
		// Should the next element be a moveto?
		let mut start_new_contour = true;
		for manipulator_group in self.manipulator_groups().iter() {
			let in_handle = manipulator_group.points[ManipulatorType::InHandle].as_ref().map(|point| point.position);
			let anchor = manipulator_group.points[ManipulatorType::Anchor].as_ref().map(|point| point.position);
			let out_handle = manipulator_group.points[ManipulatorType::OutHandle].as_ref().map(|point| point.position);

			let command = match (last_out_handle.is_some(), in_handle.is_some(), anchor.is_some()) {
				(_, _, true) if start_new_contour => 'M',
				(true, false, true) | (false, true, true) => 'Q',
				(true, true, true) => 'C',
				(false, false, true) => 'L',
				(_, false, false) => 'Z',
				_ => panic!("Invalid shape {:#?}", self),
			};

			// Complete the last curve
			if command == 'Z' {
				if last_out_handle.is_some() && first_in_handle.is_some() {
					result.push('C');
					write_positions(&mut result, [last_out_handle, first_in_handle, first_in_anchor]);
				} else if last_out_handle.is_some() || first_in_handle.is_some() {
					result.push('Q');
					write_positions(&mut result, [last_out_handle, first_in_handle, first_in_anchor]);
				} else {
					result.push('Z');
				}
			} else if command == 'M' {
				// Update the last moveto position
				(first_in_handle, first_in_anchor) = (in_handle, anchor);
				result.push(command);
				write_positions(&mut result, [None, None, anchor]);
			} else {
				result.push(command);
				write_positions(&mut result, [last_out_handle, in_handle, anchor]);
			}
			start_new_contour = command == 'Z';
			last_out_handle = out_handle;
		}
		result
	}

	/// Convert to an iter over [`bezier_rs::Bezier`] segments
	pub fn bezier_iter(&self) -> PathIter {
		PathIter {
			path: self.manipulator_groups().enumerate(),
			last_anchor: None,
			last_out_handle: None,
			last_id: None,
			first_in_handle: None,
			first_anchor: None,
			first_id: None,
			start_new_contour: true,
		}
	}
}

// ** CONVERSIONS **

impl<'a> TryFrom<&'a mut Layer> for &'a mut Subpath {
	type Error = &'static str;
	/// Convert a mutable layer into a mutable [Subpath].
	fn try_from(layer: &'a mut Layer) -> Result<&'a mut Subpath, Self::Error> {
		match &mut layer.data {
			LayerDataType::Shape(layer) => Ok(&mut layer.shape),
			// TODO Resolve converting text into a Subpath at the layer level
			// LayerDataType::Text(text) => Some(Subpath::new(path_to_shape.to_vec(), viewport_transform, true)),
			_ => Err("Did not find any shape data in the layer"),
		}
	}
}

impl<'a> TryFrom<&'a Layer> for &'a Subpath {
	type Error = &'static str;
	/// Convert a reference to a layer into a reference of a [Subpath].
	fn try_from(layer: &'a Layer) -> Result<&'a Subpath, Self::Error> {
		match &layer.data {
			LayerDataType::Shape(layer) => Ok(&layer.shape),
			// TODO Resolve converting text into a Subpath at the layer level
			// LayerDataType::Text(text) => Some(Subpath::new(path_to_shape.to_vec(), viewport_transform, true)),
			_ => Err("Did not find any shape data in the layer"),
		}
	}
}

/// An iterator over [`bezier_rs::Bezier`] segments constructable via [`Subpath::bezier_iter`]
pub struct PathIter<'a> {
	path: std::iter::Zip<core::slice::Iter<'a, u64>, core::slice::Iter<'a, ManipulatorGroup>>,

	last_anchor: Option<DVec2>,
	last_out_handle: Option<DVec2>,
	last_id: Option<u64>,

	first_in_handle: Option<DVec2>,
	first_anchor: Option<DVec2>,
	first_id: Option<u64>,

	start_new_contour: bool,
}

/// A wrapper around [`bezier_rs::Bezier`] containing also the ids for the [`ManipulatorGroup`]s where the points are from
pub struct BezierId {
	/// The internal [`bezier_rs::Bezier`].
	pub internal: bezier_rs::Bezier,
	/// The id of the [ManipulatorGroup] of the start point and, if cubic, the start handle.
	pub start: u64,
	/// The id of the [ManipulatorGroup] of the end point and, if cubic, the end handle.
	pub end: u64,
	/// The id of the [ManipulatorGroup] of the handle on a quadratic (if applicable).
	pub mid: Option<u64>,
}

impl BezierId {
	fn new(internal: bezier_rs::Bezier, start: u64, end: u64, mid: Option<u64>) -> Self {
		Self { internal, start, end, mid }
	}
}

impl<'a> Iterator for PathIter<'a> {
	type Item = BezierId;

	fn next(&mut self) -> Option<Self::Item> {
		use bezier_rs::Bezier;

		let mut result = None;

		while result.is_none() {
			let (&id, manipulator_group) = self.path.next()?;

			let in_handle = manipulator_group.points[ManipulatorType::InHandle].as_ref().map(|point| point.position);
			let anchor = manipulator_group.points[ManipulatorType::Anchor].as_ref().map(|point| point.position);
			let out_handle = manipulator_group.points[ManipulatorType::OutHandle].as_ref().map(|point| point.position);

			let mut start_new_contour = false;

			// Move to
			if anchor.is_some() && self.start_new_contour {
				// Update the last moveto position
				(self.first_in_handle, self.first_anchor) = (in_handle, anchor);
				self.first_id = Some(id);
			}
			// Cubic to
			else if let (Some(p1), Some(p2), Some(p3), Some(p4), Some(last_id)) = (self.last_anchor, self.last_out_handle, in_handle, anchor, self.last_id) {
				result = Some(BezierId::new(Bezier::from_cubic_dvec2(p1, p2, p3, p4), last_id, id, None));
			}
			// Quadratic to
			else if let (Some(p1), Some(p2), Some(p3), Some(last_id)) = (self.last_anchor, self.last_out_handle.or(in_handle), anchor, self.last_id) {
				let mid = if self.last_out_handle.is_some() { last_id } else { id };
				result = Some(BezierId::new(Bezier::from_quadratic_dvec2(p1, p2, p3), last_id, id, Some(mid)));
			}
			// Line to
			else if let (Some(p1), Some(p2), Some(last_id)) = (self.last_anchor, anchor, self.last_id) {
				result = Some(BezierId::new(Bezier::from_linear_dvec2(p1, p2), last_id, id, None));
			}
			// Close path
			else if in_handle.is_none() && anchor.is_none() {
				start_new_contour = true;
				if let (Some(last_id), Some(first_id)) = (self.last_id, self.first_id) {
					// Complete the last curve
					if let (Some(p1), Some(p2), Some(p3), Some(p4)) = (self.last_anchor, self.last_out_handle, self.first_in_handle, self.first_anchor) {
						result = Some(BezierId::new(Bezier::from_cubic_dvec2(p1, p2, p3, p4), last_id, first_id, None));
					} else if let (Some(p1), Some(p2), Some(p3)) = (self.last_anchor, self.last_out_handle.or(self.first_in_handle), self.first_anchor) {
						let mid = if self.last_out_handle.is_some() { last_id } else { first_id };
						result = Some(BezierId::new(Bezier::from_quadratic_dvec2(p1, p2, p3), last_id, first_id, Some(mid)));
					} else if let (Some(p1), Some(p2)) = (self.last_anchor, self.first_anchor) {
						result = Some(BezierId::new(Bezier::from_linear_dvec2(p1, p2), last_id, first_id, None));
					}
				}
			}

			self.start_new_contour = start_new_contour;
			self.last_out_handle = out_handle;
			self.last_anchor = anchor;
			self.last_id = Some(id);
		}
		result
	}
}

impl From<&Subpath> for BezPath {
	/// Create a [BezPath] from a [Subpath].
	fn from(subpath: &Subpath) -> Self {
		// Take manipulator groups and create path elements: line, quad or curve, or a close indicator
		let manipulator_groups_to_path_el = |first: &ManipulatorGroup, second: &ManipulatorGroup| -> (PathEl, bool) {
			match [
				&first.points[ManipulatorType::OutHandle],
				&second.points[ManipulatorType::InHandle],
				&second.points[ManipulatorType::Anchor],
			] {
				[None, None, Some(anchor)] => (PathEl::LineTo(point_to_kurbo(anchor)), false),
				[None, Some(in_handle), Some(anchor)] => (PathEl::QuadTo(point_to_kurbo(in_handle), point_to_kurbo(anchor)), false),
				[Some(out_handle), None, Some(anchor)] => (PathEl::QuadTo(point_to_kurbo(out_handle), point_to_kurbo(anchor)), false),
				[Some(out_handle), Some(in_handle), Some(anchor)] => (PathEl::CurveTo(point_to_kurbo(out_handle), point_to_kurbo(in_handle), point_to_kurbo(anchor)), false),
				[Some(out_handle), None, None] => {
					if let Some(first_anchor) = subpath.manipulator_groups().first() {
						(
							if let Some(in_handle) = &first_anchor.points[ManipulatorType::InHandle] {
								PathEl::CurveTo(
									point_to_kurbo(out_handle),
									point_to_kurbo(in_handle),
									point_to_kurbo(first_anchor.points[ManipulatorType::Anchor].as_ref().unwrap()),
								)
							} else {
								PathEl::QuadTo(point_to_kurbo(out_handle), point_to_kurbo(first_anchor.points[ManipulatorType::Anchor].as_ref().unwrap()))
							},
							true,
						)
					} else {
						(PathEl::ClosePath, true)
					}
				}
				[None, None, None] => (PathEl::ClosePath, true),
				_ => panic!("Invalid path element {:#?}", subpath),
			}
		};

		if subpath.manipulator_groups().is_empty() {
			return BezPath::new();
		}

		let mut bez_path = vec![];
		let mut start_new_shape = true;

		for elements in subpath.manipulator_groups().windows(2) {
			let first = &elements[0];
			let second = &elements[1];

			// Tell kurbo cursor to move to the first anchor
			if start_new_shape {
				if let Some(anchor) = &first.points[ManipulatorType::Anchor] {
					bez_path.push(PathEl::MoveTo(point_to_kurbo(anchor)));
				}
			}

			// Create a path element from our first and second manipulator groups in the window
			let (path_el, should_start_new_shape) = manipulator_groups_to_path_el(first, second);
			start_new_shape = should_start_new_shape;
			bez_path.push(path_el);
			if should_start_new_shape && bez_path.last().filter(|&&el| el == PathEl::ClosePath).is_none() {
				bez_path.push(PathEl::ClosePath)
			}
		}

		BezPath::from_vec(bez_path)
	}
}

impl<T: Iterator<Item = PathEl>> From<T> for Subpath {
	/// Create a Subpath from a [BezPath].
	fn from(path: T) -> Self {
		let mut subpath = Subpath::new();
		for path_el in path {
			match path_el {
				PathEl::MoveTo(p) => {
					subpath.manipulator_groups_mut().push_end(ManipulatorGroup::new_with_anchor(kurbo_point_to_dvec2(p)));
				}
				PathEl::LineTo(p) => {
					subpath.manipulator_groups_mut().push_end(ManipulatorGroup::new_with_anchor(kurbo_point_to_dvec2(p)));
				}
				PathEl::QuadTo(p0, p1) => {
					subpath.manipulator_groups_mut().push_end(ManipulatorGroup::new_with_anchor(kurbo_point_to_dvec2(p1)));
					subpath.manipulator_groups_mut().last_mut().unwrap().points[ManipulatorType::InHandle] = Some(ManipulatorPoint::new(kurbo_point_to_dvec2(p0), ManipulatorType::InHandle));
				}
				PathEl::CurveTo(p0, p1, p2) => {
					subpath.manipulator_groups_mut().last_mut().unwrap().points[ManipulatorType::OutHandle] = Some(ManipulatorPoint::new(kurbo_point_to_dvec2(p0), ManipulatorType::OutHandle));
					subpath.manipulator_groups_mut().push_end(ManipulatorGroup::new_with_anchor(kurbo_point_to_dvec2(p2)));
					subpath.manipulator_groups_mut().last_mut().unwrap().points[ManipulatorType::InHandle] = Some(ManipulatorPoint::new(kurbo_point_to_dvec2(p1), ManipulatorType::InHandle));
				}
				PathEl::ClosePath => {
					subpath.manipulator_groups_mut().push_end(ManipulatorGroup::closed());
				}
			}
		}

		subpath
	}
}

#[inline]
fn point_to_kurbo(point: &ManipulatorPoint) -> kurbo::Point {
	kurbo::Point::new(point.position.x, point.position.y)
}

#[inline]
fn kurbo_point_to_dvec2(point: kurbo::Point) -> DVec2 {
	DVec2::new(point.x, point.y)
}
