use super::constants::ControlPointType;
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// VectorControlPoint represents any grabbable point, anchor or handle
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct VectorControlPoint {
	// The sibling element if this is a handle
	pub position: glam::DVec2,
	// The type of manipulator this point is
	pub manipulator_type: ControlPointType,
	// Can this control point be selected?
	#[serde(skip_serializing)]
	pub can_be_selected: bool,
	// Is this point currently selected?
	#[serde(skip_serializing)]
	pub is_selected: bool,
}

impl Default for VectorControlPoint {
	fn default() -> Self {
		Self {
			position: DVec2::ZERO,
			manipulator_type: ControlPointType::Anchor,
			can_be_selected: true,
			is_selected: false,
		}
	}
}

impl VectorControlPoint {
	// Initialize a new control point
	pub fn new(position: glam::DVec2, manipulator_type: ControlPointType) -> Self {
		Self {
			position,
			manipulator_type,
			can_be_selected: true,
			is_selected: false,
		}
	}

	/// Sets if this point is selected and updates the overlay to represent that
	pub fn set_selected(&mut self, selected: bool) {
		self.is_selected = selected;
	}

	/// apply given transform
	pub fn transform(&mut self, delta: &DAffine2) {
		self.position = delta.transform_point2(self.position);
	}

	/// Move by a delta amount
	pub fn move_by(&mut self, delta: &DVec2) {
		self.position += *delta;
	}
}
