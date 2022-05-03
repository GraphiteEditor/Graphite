use crate::{LayerId, layers::layer_info::{Layer, LayerDataType}};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, PathEl, Rect, Shape};
use serde::{Deserialize, Serialize};
type AnchorId = u64;
/// VectorShape represents a single vector shape, containing many anchors
/// For each kurbo path we keep a VectorShape which contains the handles and anchors for that path
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VectorShape {
	// TODO Have VectorShape work like folders (use @TrueDoctors generic uuid magic)
	/// The ids for the anchors
	//pub anchor_ids: Vec<AnchorId>,
	/// The path to the shape layer
	pub layer_path: Vec<LayerId>,
	/// Vec of anchors, each consisting of the control points / handles
	pub anchors: Vec<VectorAnchor>,
	/// If the compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	/// My no longer be needed.
	pub transform: DAffine2,
}

// TODO Implement iterator for VectorShape

impl VectorShape {
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, closed: bool) -> Self {
		VectorShape {
			layer_path,
			closed,
			transform,
			..Default::default()
		}
	}

	pub fn from_kurbo_shape<T: Shape>(shape: &T) -> Self {
		shape.path_elements(0.1).into()
	}

	pub fn move_selected(&mut self, delta: DVec2, relative: bool) {
		// TODO Reimplement this function properly
		for anchor in self.selected_anchors_mut() {
			if anchor.is_anchor_selected() {
				// anchor.move_selected_points(anchor.control_points_mut(), delta, relative);
			}
		}
	}

	pub fn delete_selected(&mut self) {
		// TODO Reimplement this function properly
		for anchor in self.selected_anchors_mut() {
			if anchor.is_anchor_selected() {}
		}
	}

	pub fn add_point(&mut self, nearest_point_on_curve: DVec2) {
		// TODO Implement this function properly
		for anchor in self.selected_anchors_mut() {
			if anchor.is_anchor_selected() {
				// anchor.add_point(anchor.control_points_mut(), nearest_point_on_curve);
			}
		}
	}

	/// Select an anchor
	pub fn select_anchor(&mut self, anchor_index: usize) -> &mut VectorAnchor {
		self.anchors[anchor_index].select_point(ControlPointType::Anchor as usize, true);
		&mut self.anchors[anchor_index]
	}

	/// The last anchor in the shape thus far
	pub fn select_last_anchor(&mut self) -> &mut VectorAnchor {
		let last_index = self.anchors.len() - 1;
		self.anchors[last_index].select_point(ControlPointType::Anchor as usize, true);
		&mut self.anchors[last_index]
	}

	/// Deselect an anchor
	pub fn deselect_anchor(&mut self, anchor_index: usize) {
		self.anchors[anchor_index].clear_selected_points();
		self.anchors[anchor_index].select_point(ControlPointType::Anchor as usize, false);
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self) {
		for anchor in self.anchors.iter_mut() {
			anchor.select_point(ControlPointType::Anchor as usize, true);
		}
	}

	/// Clear all the selected anchors, and clear the selected points on the anchors
	pub fn clear_selected_anchors(&mut self) {
		for anchor in self.anchors.iter_mut() {
			anchor.clear_selected_points();
		}
	}

	/// Return all the selected anchors by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors.iter().enumerate().filter_map(|(_, anchor)| if anchor.is_anchor_selected() { Some(anchor) } else { None })
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors
			.iter_mut()
			.enumerate()
			.filter_map(|(_, anchor)| if anchor.is_anchor_selected() { Some(anchor) } else { None })
	}

	/// Return a mutable interator of the anchors regardless of selection
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
impl <'a>TryFrom<&'a mut Layer> for &'a mut VectorShape {
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
impl <'a>TryFrom<&'a Layer> for &'a VectorShape {
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

		let point = vector_shape.anchors[0].points[0].as_ref().unwrap().position;
		let mut bez_path = vec![PathEl::MoveTo((point.x, point.y).into())];

		for elements in vector_shape.anchors.windows(2) {
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
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p)));
				}
				PathEl::LineTo(p) => {
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p)));
				}
				PathEl::QuadTo(p0, p1) => {
					vector_shape.anchors.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p0), ControlPointType::Handle2));
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p1)));
					vector_shape.anchors.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p0), ControlPointType::Handle1));
				}
				PathEl::CurveTo(p0, p1, p2) => {
					vector_shape.anchors.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p0), ControlPointType::Handle2));
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p2)));
					vector_shape.anchors.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p1), ControlPointType::Handle1));
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
fn kurbo_point_to_DVec2(p: kurbo::Point) -> DVec2 {
	DVec2::new(p.x, p.y)
}
