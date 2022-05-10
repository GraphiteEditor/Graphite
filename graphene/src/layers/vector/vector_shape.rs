use crate::layers::{
	id_storage::UniqueElements,
	layer_info::{Layer, LayerDataType},
	style::PathStyle,
	LayerId,
};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, PathEl, Rect, Shape};
use serde::{Deserialize, Serialize};

/// VectorShape represents a single vector shape, containing many anchors
/// For each kurbo path we keep a VectorShape which contains the handles and anchors for that path
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VectorShape {
	/// The path to the shape layer
	pub layer_path: Vec<LayerId>,
	/// Vec of anchors, each consisting of the control points / handles
	pub anchors: UniqueElements<VectorAnchor>,
	/// If the compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	/// May no longer be needed.
	pub transform: DAffine2,
	/// Is this shape selected
	pub selected: bool,
}

// TODO Implement iterator for VectorShape

impl VectorShape {
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, closed: bool) -> Self {
		VectorShape {
			layer_path,
			closed,
			transform,
			selected: true,
			..Default::default()
		}
	}

	pub fn from_kurbo_shape<T: Shape>(shape: &T) -> Self {
		shape.path_elements(0.1).into()
	}

	/// constructs a rectangle with `p1` as the lower left and `p2` as the top right
	pub fn new_rect(p1: DVec2, p2: DVec2) -> Self {
		VectorShape {
			layer_path: vec![],
			anchors: vec![
				VectorAnchor::new(p1),
				VectorAnchor::new(DVec2::new(p1.x, p2.y)),
				VectorAnchor::new(p2),
				VectorAnchor::new(DVec2::new(p2.x, p1.y)),
			]
			.into_iter()
			.collect(),
			closed: true,
			transform: DAffine2::IDENTITY,
			selected: false,
		}
	}

	/// constructs an ngon
	/// `radius` is the distance from the center to any vertex, or the radius of the circle the ngon may be inscribed inside
	pub fn new_ngon(center: DVec2, sides: u64, radius: f64) -> Self {
		let mut anchors = vec![];
		for i in 0..sides {
			let angle = (i as f64) * std::f64::consts::TAU / (sides as f64);
			anchors.push(VectorAnchor::new(DVec2::new(center.x + radius * f64::cos(angle), center.y + radius * f64::sin(angle))));
		}
		VectorShape {
			layer_path: vec![],
			anchors: anchors.into_iter().collect(),
			closed: true,
			transform: DAffine2::IDENTITY,
			selected: false,
		}
	}

	/// constructs a line from `p1` to `p2`
	pub fn new_line(p1: DVec2, p2: DVec2) -> Self {
		VectorShape {
			layer_path: vec![],
			anchors: vec![VectorAnchor::new(p1), VectorAnchor::new(p2)].into_iter().collect(),
			closed: false,
			transform: DAffine2::IDENTITY,
			selected: false,
		}
	}

	pub fn new_poly_line<T: Into<glam::DVec2>>(points: Vec<T>) -> Self {
		let mut p_line = VectorShape {
			layer_path: vec![],
			anchors: UniqueElements::default(),
			closed: false,
			transform: DAffine2::IDENTITY,
			selected: false,
		};
		points
			.into_iter()
			.enumerate()
			.for_each(|(local_id, point)| match p_line.anchors.add(VectorAnchor::new(point.into()), None, -1) {
				_ => (),
			});
		p_line
	}

	pub fn move_selected(&mut self, delta: DVec2, relative: bool) {
		self.selected_anchors_mut().for_each(|anchor| anchor.move_selected_points(relative, &DAffine2::from_translation(delta)));
	}

	pub fn delete_selected(&mut self) {
		self.anchors = self.anchors.clone().into_iter().filter(|anchor| !anchor.is_anchor_selected()).collect();
	}

	/// Select an anchor
	pub fn select_anchor_index(&mut self, anchor_index: usize) -> &mut VectorAnchor {
		self.anchors[anchor_index].select_point(ControlPointType::Anchor as usize, true);
		&mut self.anchors[anchor_index]
	}

	pub fn add_point(&mut self, nearest_point_on_curve: DVec2) {
		// TODO Implement this function properly
		for anchor in self.selected_anchors_mut() {
			if anchor.is_anchor_selected() {
				// anchor.add_point(anchor.control_points_mut(), nearest_point_on_curve);
			}
		}
	}

	/// Select an anchor by id
	pub fn select_anchor(&mut self, anchor_id: u64) -> Option<&mut VectorAnchor> {
		// TODO test if looking this up by index actually works
		if let Some(anchor) = self.anchors.by_id_mut(anchor_id) {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// Select an anchor by index
	pub fn select_anchor_by_index(&mut self, anchor_index: usize) -> Option<&mut VectorAnchor> {
		// TODO test if looking this up by index actually works
		if let Some(anchor) = self.anchors.by_index_mut(anchor_index) {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// The last anchor in the shape thus far
	pub fn select_last_anchor(&mut self) -> Option<&mut VectorAnchor> {
		if let Some(anchor) = self.anchors.last_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
			return Some(anchor);
		}
		None
	}

	/// Deselect an anchor
	pub fn deselect_anchor(&mut self, anchor_id: u64) {
		// TODO test if looking this up by index actually works
		if let Some(anchor) = self.anchors.by_id_mut(anchor_id) {
			anchor.clear_selected_points();
			anchor.select_point(ControlPointType::Anchor as usize, false);
		}
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self) {
		for anchor in self.anchors.values_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
		}
	}

	/// Clear all the selected anchors, and clear the selected points on the anchors
	pub fn clear_selected_anchors(&mut self) {
		for anchor in self.anchors.values_mut() {
			anchor.clear_selected_points();
		}
	}

	/// Return all the selected anchors by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors.iter().filter(|anchor| anchor.is_anchor_selected())
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors.iter_mut().filter(|anchor| anchor.is_anchor_selected())
	}

	pub fn set_selected(&mut self, selected: bool) {
		self.selected = selected;
	}

	/// Return a mutable iterator of the anchors regardless of selection
	pub fn anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors.iter_mut()
	}

	/// Place point in local space in relation to this shape's transform
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}

	/// TODO: remove kurbo from below implementations

	pub fn apply_affine(&mut self, affine: DAffine2) {
		let mut transformed = <&Self as Into<BezPath>>::into(self);
		transformed.apply_affine(glam_to_kurbo(affine));
		self.anchors = Into::<VectorShape>::into(transformed.iter()).anchors;
	}

	pub fn bounding_box(&self) -> Rect {
		<&Self as Into<BezPath>>::into(self).bounding_box()
	}

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
		if vector_shape.anchors.is_empty() {
			return BezPath::new();
		}

		let point = vector_shape.anchors.by_index(0).unwrap().points[ControlPointType::Anchor].as_ref().unwrap().position;
		let mut bez_path = vec![PathEl::MoveTo((point.x, point.y).into())];

		for elements in vector_shape.anchors.values().windows(2) {
			let first = &elements[0];
			let second = &elements[1];
			let new_segment = match [&first.points[2], &second.points[1], &second.points[0]] {
				[None, None, Some(p)] => PathEl::LineTo(point_to_kurbo(p)),
				[None, Some(a), Some(p)] => PathEl::QuadTo(point_to_kurbo(a), point_to_kurbo(p)),
				[Some(a1), Some(a2), Some(p)] => PathEl::CurveTo(point_to_kurbo(a1), point_to_kurbo(a2), point_to_kurbo(p)),
				_ => panic!("unexpected path found"),
			};
			bez_path.push(new_segment);
		}
		if vector_shape.closed {
			bez_path.push(PathEl::ClosePath);
		}

		log::debug!("To Bezpath: {:?}", bez_path);
		BezPath::from_vec(bez_path)
	}
}

/// Create a VectorShape from a BezPath
impl<T: Iterator<Item = PathEl>> From<T> for VectorShape {
	fn from(path: T) -> Self {
		let mut vector_shape = VectorShape::new(vec![], DAffine2::IDENTITY, false);
		let mut current_closed = true;
		let mut closed_flag = false;
		for path_el in path {
			match path_el {
				PathEl::MoveTo(p) => {
					if !current_closed {
						closed_flag = false;
					}
					current_closed = false;
					vector_shape.anchors.add(VectorAnchor::new(kurbo_point_to_dvec2(p)), None, -1);
				}
				PathEl::LineTo(p) => {
					vector_shape.anchors.add(VectorAnchor::new(kurbo_point_to_dvec2(p)), None, -1);
				}
				PathEl::QuadTo(p0, p1) => {
					vector_shape.anchors.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::Handle2));
					vector_shape.anchors.add(VectorAnchor::new(kurbo_point_to_dvec2(p1)), None, -1);
					vector_shape.anchors.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::Handle1));
				}
				PathEl::CurveTo(p0, p1, p2) => {
					vector_shape.anchors.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p0), ControlPointType::Handle2));
					vector_shape.anchors.add(VectorAnchor::new(kurbo_point_to_dvec2(p2)), None, -1);
					vector_shape.anchors.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_dvec2(p1), ControlPointType::Handle1));
				}
				PathEl::ClosePath => {
					current_closed = true;
					closed_flag = true;
				}
			}
		}
		// a VectorShape is closed if and only if every subpath is closed
		vector_shape.closed = closed_flag;
		vector_shape
	}
}

///*Kurbo adaptors */
#[inline]
fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

#[inline]
fn point_to_kurbo(x: &VectorControlPoint) -> kurbo::Point {
	kurbo::Point::new(x.position.x, x.position.y)
}

#[inline]
fn kurbo_point_to_dvec2(p: kurbo::Point) -> DVec2 {
	DVec2::new(p.x, p.y)
}
