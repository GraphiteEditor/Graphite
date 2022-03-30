use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint};
use crate::{
	consts::COLOR_ACCENT,
	document::DocumentMessageHandler,
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

use graphene::{
	color::Color,
	layers::{
		layer_info::LayerDataType,
		style::{self, Fill, Stroke},
	},
	LayerId, Operation,
};

use glam::{DAffine2, DVec2};
use kurbo::{BezPath, PathEl};
use std::collections::HashSet;
use std::collections::VecDeque;

/// VectorShape represents a single kurbo shape and maintains a parallel data structure
/// For each kurbo path we keep a VectorShape which contains the handles and anchors for that path
#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorShape {
	/// The path to the shape layer
	pub layer_path: Vec<LayerId>,
	/// The anchors that are made up of the control points / handles
	pub anchors: Vec<VectorAnchor>,
	/// If the compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
	// Indices for the most recent select point anchors
	pub selected_anchor_indices: HashSet<usize>,
}
type IndexedEl = (usize, kurbo::PathEl);

impl VectorShape {
	pub fn new(layer_path: Vec<LayerId>, transform: DAffine2, bez_path: &BezPath, closed: bool, responses: &mut VecDeque<Message>) -> Self {
		let mut shape = VectorShape {
			layer_path,
			closed,
			transform,
			..Default::default()
		};

		shape
	}

	/// Select an anchor
	pub fn select_anchor(&mut self, anchor_index: usize) -> &mut VectorAnchor {
		self.selected_anchor_indices.insert(anchor_index);
		&mut self.anchors[anchor_index]
	}

	/// The last anchor in the shape thus far
	pub fn select_last_anchor(&mut self) -> &mut VectorAnchor {
		let last_index = self.anchors.len() - 1;
		self.selected_anchor_indices.insert(last_index);
		&mut self.anchors[last_index]
	}

	/// Deselect an anchor
	pub fn deselect_anchor(&mut self, anchor_index: usize, responses: &mut VecDeque<Message>) {
		self.anchors[anchor_index].clear_selected_points(responses);
		self.selected_anchor_indices.remove(&anchor_index);
	}

	/// Select all the anchors in this shape
	pub fn select_all_anchors(&mut self, responses: &mut VecDeque<Message>) {
		for (index, anchor) in self.anchors.iter_mut().enumerate() {
			self.selected_anchor_indices.insert(index);
			anchor.select_point(0, true, responses);
		}
	}

	/// Clear all the selected anchors, and clear the selected points on the anchors
	pub fn clear_selected_anchors(&mut self, responses: &mut VecDeque<Message>) {
		for anchor_index in self.selected_anchor_indices.iter() {
			self.anchors[*anchor_index].clear_selected_points(responses);
		}
		self.selected_anchor_indices.clear();
	}

	/// Return all the selected anchors by reference
	pub fn selected_anchors(&self) -> impl Iterator<Item = &VectorAnchor> {
		self.anchors
			.iter()
			.enumerate()
			.filter_map(|(index, anchor)| if self.selected_anchor_indices.contains(&index) { Some(anchor) } else { None })
	}

	/// Return all the selected anchors, mutable
	pub fn selected_anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors
			.iter_mut()
			.enumerate()
			.filter_map(|(index, anchor)| if self.selected_anchor_indices.contains(&index) { Some(anchor) } else { None })
	}

	/// Return a mutable interator of the anchors regardless of selection
	pub fn anchors_mut(&mut self) -> impl Iterator<Item = &mut VectorAnchor> {
		self.anchors.iter_mut()
	}

	/// Place point in local space in relation to this shape's transform
	fn to_local_space(&self, point: kurbo::Point) -> DVec2 {
		self.transform.transform_point2(DVec2::from((point.x, point.y)))
	}
}
