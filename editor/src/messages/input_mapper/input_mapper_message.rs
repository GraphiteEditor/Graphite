use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::input_mapper::utility_types::input_mouse::MouseButton;
use crate::messages::prelude::*;

#[impl_message(Message, KeyMappingMessage, Lookup)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum InputMapperMessage {
	// Sub-messages
	#[child]
	KeyDown(Key),
	#[child]
	KeyUp(Key),
	#[child]
	KeyDownNoRepeat(Key),
	#[child]
	KeyUpNoRepeat(Key),
	#[child]
	DoubleClick(MouseButton),

	// Messages
	PointerMove,
	PointerShake,
	WheelScroll,
}
