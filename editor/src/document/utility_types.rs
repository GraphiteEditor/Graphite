pub use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};

use graphene::document::Document as GrapheneDocument;
use graphene::LayerId;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerMetadata>);

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Clone, Debug)]
pub enum VectorManipulatorSegment {
	Line(DVec2, DVec2),
	Quad(DVec2, DVec2, DVec2),
	Cubic(DVec2, DVec2, DVec2, DVec2),
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorShape {
	/// The path to the layer
	pub layer_path: Vec<LayerId>,
	/// The outline of the shape
	pub path: kurbo::BezPath,
	/// The segments containing the control points / manipulator handles
	pub segments: Vec<VectorManipulatorSegment>,
	/// The control points / manipulator handles
	pub points: Vec<VectorManipulatorAnchor>,
	/// The compound Bezier curve is closed
	pub closed: bool,
	/// The transformation matrix to apply
	pub transform: DAffine2,
}

#[derive(PartialEq, Clone, Debug, Copy, Default)]
pub struct VectorManipulatorPoint {
	// The associated position in the BezPath
	pub element_id: usize,
	// The sibling element if this is a handle
	pub position: glam::DVec2,
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorManipulatorAnchor {
	// The associated position in the BezPath
	pub point: VectorManipulatorPoint,
	// Does this anchor point have a path close element we also needs to move?
	pub close_element_id: Option<usize>,
	// Should we mirror the handles
	pub handle_mirroring: bool,
	// Anchor handles
	pub handles: (Option<VectorManipulatorPoint>, Option<VectorManipulatorPoint>),
}

impl VectorManipulatorAnchor {
	pub fn closest_handle_or_anchor(&self, target: glam::DVec2) -> &VectorManipulatorPoint {
		let mut closest_point: &VectorManipulatorPoint = &self.point;
		let mut distance = self.point.position.distance_squared(target);
		let (handle1, handle2) = &self.handles;
		if let Some(handle1) = handle1 {
			let handle1_dist = handle1.position.distance_squared(target);
			if distance > handle1_dist {
				distance = handle1_dist;
				closest_point = handle1;
			}
		}

		if let Some(handle2) = handle2 {
			let handle2_dist = handle2.position.distance_squared(target);
			if distance > handle2_dist {
				closest_point = handle2;
			}
		}

		closest_point
	}

	/// Angle bewtween handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let (Some(h1), Some(h2)) = &self.handles {
			return (self.point.position - h1.position).angle_between(self.point.position - h2.position);
		}
		0.0
	}

	pub fn opposing_handle(&self, handle: &VectorManipulatorPoint) -> &Option<VectorManipulatorPoint> {
		if Some(handle) == self.handles.0.as_ref() {
			&self.handles.1
		} else {
			&self.handles.0
		}
	}
}

impl VectorManipulatorPoint {
	pub(crate) fn clone(&self) -> VectorManipulatorPoint {
		VectorManipulatorPoint {
			element_id: self.element_id,
			position: self.position,
		}
	}
}
