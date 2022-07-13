use super::constants::ManipulatorType;
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// [ManipulatorPoint] represents any editable Bezier point, either an anchor or handle
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct ManipulatorPoint {
	/// The sibling element if this is a handle
	pub position: glam::DVec2,
	/// The type of manipulator this point is
	pub manipulator_type: ManipulatorType,

	#[serde(skip)]
	/// The state specific to the editor
	// TODO Remove this from Graphene, editor state should be stored in the frontend if possible.
	pub editor_state: ManipulatorPointEditorState,
}

impl Default for ManipulatorPoint {
	fn default() -> Self {
		Self {
			position: DVec2::ZERO,
			manipulator_type: ManipulatorType::Anchor,
			editor_state: ManipulatorPointEditorState::default(),
		}
	}
}

impl ManipulatorPoint {
	/// Initialize a new [ManipulatorPoint].
	pub fn new(position: glam::DVec2, manipulator_type: ManipulatorType) -> Self {
		assert!(position.is_finite(), "tried to create point with non finite position");
		Self {
			position,
			manipulator_type,
			editor_state: ManipulatorPointEditorState::default(),
		}
	}

	/// Sets this [ManipulatorPoint] to a chosen selection state.
	pub fn set_selected(&mut self, selected: bool) {
		self.editor_state.is_selected = selected;
	}

	/// Whether this [ManipulatorPoint] is currently selected.
	pub fn is_selected(&self) -> bool {
		self.editor_state.is_selected
	}

	/// Apply given transform to this point
	pub fn transform(&mut self, delta: &DAffine2) {
		self.position = delta.transform_point2(self.position);
		assert!(self.position.is_finite(), "tried to transform point to non finite position");
	}

	/// Move by a delta amount
	pub fn move_by(&mut self, delta: &DVec2) {
		self.position += *delta;
		assert!(self.position.is_finite(), "tried to move point to non finite position");
	}
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ManipulatorPointEditorState {
	/// Whether or not this manipulator point can be selected.
	pub can_be_selected: bool,
	/// Whether or not this manipulator point is currently selected.
	pub is_selected: bool,
}

impl Default for ManipulatorPointEditorState {
	fn default() -> Self {
		Self {
			can_be_selected: true,
			is_selected: false,
		}
	}
}
