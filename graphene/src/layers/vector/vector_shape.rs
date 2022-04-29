use crate::{layers::style::PathStyle, LayerId};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, PathEl, Rect, Shape};
use serde::{Deserialize, Serialize};

/// VectorShape represents a single kurbo shape and maintains a parallel data structure
/// For each kurbo path we keep a VectorShape which contains the handles and anchors for that path
/// TODO remove clonable, we don't want any duplicates
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VectorShape {
	/// The path to the shape layer
	pub layer_path: Vec<LayerId>,
	/// The anchors that are made up of the control points / handles
	pub anchors: Vec<VectorAnchor>,
	/// If the compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
}

impl VectorShape {
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, closed: bool) -> Self {
		let mut shape = VectorShape {
			layer_path,
			closed,
			transform,
			..Default::default()
		};

		shape
	}

	pub fn from_kurbo_shape<T: Shape>(shape: &T) -> Self {
		shape.path_elements(0.1).into()
	}

	/// constructs a rectangle with `p1` as the lower left and `p2` as the top right
	pub fn new_rect(p1: DVec2, p2: DVec2) -> Self {
		VectorShape {
			layer_path: vec![],
			anchors: vec![
				VectorAnchor::new(p1, 0),
				VectorAnchor::new(DVec2::new(p1.x, p2.y), 1),
				VectorAnchor::new(p2, 2),
				VectorAnchor::new(DVec2::new(p2.x, p1.y), 3),
			],
			closed: true,
			transform: DAffine2::IDENTITY,
		}
	}

	pub fn move_selected(&mut self, delta: DVec2, relative: bool) {
		self.selected_anchors_mut().for_each(|anchor| anchor.move_selected_points(relative, &DAffine2::from_translation(delta)));
	}

	pub fn delete_selected(&mut self) {
		self.anchors = self.anchors.clone().into_iter().filter(|anchor| !anchor.is_anchor_selected()).collect();
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
		self.anchors
			.iter()
			.enumerate()
			.filter_map(|(index, anchor)| if anchor.is_anchor_selected() { Some(anchor) } else { None })
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors.iter_mut().filter(|anchor| anchor.is_anchor_selected())
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

impl<T: Iterator<Item = PathEl>> From<T> for VectorShape {
	fn from(path: T) -> Self {
		let mut anchor_id = 0;
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
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p), anchor_id));
					anchor_id += 1;
				}
				PathEl::LineTo(p) => {
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p), anchor_id));
					anchor_id += 1;
				}
				PathEl::QuadTo(p0, p1) => {
					vector_shape.anchors.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p0), ControlPointType::Handle2));
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p1), anchor_id));
					vector_shape.anchors.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p0), ControlPointType::Handle1));
					anchor_id += 1;
				}
				PathEl::CurveTo(p0, p1, p2) => {
					vector_shape.anchors.last_mut().unwrap().points[2] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p0), ControlPointType::Handle2));
					vector_shape.anchors.push(VectorAnchor::new(kurbo_point_to_DVec2(p2), anchor_id));
					vector_shape.anchors.last_mut().unwrap().points[1] = Some(VectorControlPoint::new(kurbo_point_to_DVec2(p1), ControlPointType::Handle1));
					anchor_id += 1;
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
