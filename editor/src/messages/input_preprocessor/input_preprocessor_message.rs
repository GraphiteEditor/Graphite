use crate::messages::input_mapper::utility_types::keyboard::{Key, ModifierKeys};
use crate::messages::input_mapper::utility_types::pointer::EditorPointerState;
use crate::messages::prelude::*;

#[impl_message(Message, InputPreprocessor)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum InputPreprocessorMessage {
	DoubleClick {
		editor_mouse_state: EditorPointerState,
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
		editor_mouse_state: EditorPointerState,
		modifier_keys: ModifierKeys,
	},
	PointerMove {
		editor_mouse_state: EditorPointerState,
		modifier_keys: ModifierKeys,
	},
	/// The pointer moved over GUI that covers the canvas, the node graph say: only the position is recorded, for
	/// presence and the like, and no tool hears of it.
	PointerHover {
		editor_mouse_state: EditorPointerState,
	},
	PointerUp {
		editor_mouse_state: EditorPointerState,
		modifier_keys: ModifierKeys,
	},
	PointerShake {
		editor_mouse_state: EditorPointerState,
		modifier_keys: ModifierKeys,
	},
	CurrentTime {
		timestamp: u64,
	},
	WheelScroll {
		editor_mouse_state: EditorPointerState,
		modifier_keys: ModifierKeys,
	},
}
