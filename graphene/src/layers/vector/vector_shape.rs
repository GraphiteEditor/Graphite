use crate::LayerId;

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};

use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, PathEl};
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
		self.anchors
			.iter_mut()
			.enumerate()
			.filter_map(|(index, anchor)| if anchor.is_anchor_selected() { Some(anchor) } else { None })
	}

	/// Return a mutable interator of the anchors regardless of selection
	pub fn anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors.iter_mut()
	}

	/// Place point in local space in relation to this shape's transform
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}

	pub fn apply_affine(&mut self, affine: DAffine2) {
		<&Self as Into<BezPath>>::into(self).apply_affine(glam_to_kurbo(affine));
	}
}

impl From<&VectorShape> for BezPath {
	fn from(vector_shape: &VectorShape) -> Self {
		if vector_shape.anchors.is_empty() {
			return BezPath::new();
		}
		let point_to_kurbo = |x: &VectorControlPoint| kurbo::Point::new(x.position.x, x.position.y);
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

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}
