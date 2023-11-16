use crate::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
use crate::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ViewportBounds};
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum InputPreprocessorMessage {
	BoundsOfViewports { bounds_of_viewports: Vec<ViewportBounds> },
	DoubleClick { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	KeyDown { key: Key, key_repeat: bool, modifier_keys: ModifierKeys },
	KeyUp { key: Key, key_repeat: bool, modifier_keys: ModifierKeys },
	PointerDown { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	PointerMove { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	PointerUp { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	WheelScroll { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
}
