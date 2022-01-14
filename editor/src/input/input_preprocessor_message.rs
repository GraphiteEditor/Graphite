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
	BoundsOfViewports(Vec<ViewportBounds>),
	KeyDown(Key, ModifierKeys),
	KeyUp(Key, ModifierKeys),
	MouseDown(EditorMouseState, ModifierKeys),
	MouseMove(EditorMouseState, ModifierKeys),
	MouseScroll(EditorMouseState, ModifierKeys),
	MouseUp(EditorMouseState, ModifierKeys),
}
