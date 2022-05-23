use super::constants::ControlPointType;
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// VectorControlPoint represents any grabbable point, anchor or handle
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct VectorControlPoint {
	/// The sibling element if this is a handle
	pub position: glam::DVec2,
	/// The type of manipulator this point is
	pub manipulator_type: ControlPointType,

	#[serde(skip)]
	/// The state specific to the editor
	pub editor_state: VectorControlPointState,
}

impl Default for VectorControlPoint {
	fn default() -> Self {
		Self {
			position: DVec2::ZERO,
			manipulator_type: ControlPointType::Anchor,
			editor_state: VectorControlPointState::default(),
		}
	}
}

impl VectorControlPoint {
	/// Initialize a new control point
	pub fn new(position: glam::DVec2, manipulator_type: ControlPointType) -> Self {
		Self {
			position,
			manipulator_type,
			editor_state: VectorControlPointState::default(),
		}
	}

	/// Sets if this point is selected
	pub fn set_selected(&mut self, selected: bool) {
		self.editor_state.is_selected = selected;
	}

	/// Apply given transform to this point
	pub fn transform(&mut self, delta: &DAffine2) {
		self.position = delta.transform_point2(self.position);
	}

	/// Move by a delta amount
	pub fn move_by(&mut self, delta: &DVec2) {
		self.position += *delta;
	}
}


#[derive(PartialEq, Clone, Debug)]
pub struct VectorControlPointState {
	/// If this control point can be selected
	pub can_be_selected: bool,
	/// Is this control point currently selected
	pub is_selected: bool,
}

impl Default for VectorControlPointState {
	fn default() -> Self {
		Self {
			can_be_selected: true,
			is_selected: false,
		}
	}
}
