use crate::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
use crate::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ViewportBounds};
use crate::messages::prelude::*;

use core::time::Duration;

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum InputPreprocessorMessage {
	BoundsOfViewports {
		bounds_of_viewports: Vec<ViewportBounds>,
	},
	DoubleClick {
		editor_mouse_state: EditorMouseState,
		modifier_keys: ModifierKeys,
	},
	KeyDown {
		key: Key,
		key_repeat: bool,
		modifier_keys: ModifierKeys,
	},
	KeyUp {
		key: Key,
		key_repeat: bool,
		modifier_keys: ModifierKeys,
	},
	PointerDown {
		editor_mouse_state: EditorMouseState,
		modifier_keys: ModifierKeys,
	},
	PointerMove {
		editor_mouse_state: EditorMouseState,
		modifier_keys: ModifierKeys,
		pen_data: Option<PenMoveState>,
	},

	PointerUp {
		editor_mouse_state: EditorMouseState,
		modifier_keys: ModifierKeys,
	},
	FrameTimeAdvance {
		timestamp: Duration,
	},
	WheelScroll {
		editor_mouse_state: EditorMouseState,
		modifier_keys: ModifierKeys,
	},
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PenMoveState {
	pub pressure: f32,
	pub tangential_pressure: f32,
	pub tilt_x: i8,
	pub tilt_y: i8,
	pub twist: u16,
	pub pointer_id: u32,
}
