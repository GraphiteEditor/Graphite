use super::input_preprocessor::ModifierKeys;
use super::keyboard::Key;
use super::mouse::{EditorMouseState, ViewportBounds};
use crate::message_prelude::*;

#[doc(inline)]
pub use graphene::DocumentResponse;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum InputPreprocessorMessage {
	BoundsOfViewports { bounds_of_viewports: Vec<ViewportBounds> },
	KeyDown { key: Key, modifier_keys: ModifierKeys },
	KeyUp { key: Key, modifier_keys: ModifierKeys },
	MouseDown { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	MouseMove { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	MouseScroll { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
	MouseUp { editor_mouse_state: EditorMouseState, modifier_keys: ModifierKeys },
}
