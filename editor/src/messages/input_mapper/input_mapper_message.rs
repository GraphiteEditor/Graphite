use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, KeyMappingMessage, Lookup)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum InputMapperMessage {
	// Sub-messages
	#[remain::unsorted]
	#[child]
	KeyDown(Key),
	#[remain::unsorted]
	#[child]
	KeyUp(Key),

	// Messages
	DoubleClick,
	PointerMove,
	WheelScroll,
}
