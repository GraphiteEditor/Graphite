use super::keyboard::Key;
use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
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
	MouseScroll,
	PointerMove,
}
