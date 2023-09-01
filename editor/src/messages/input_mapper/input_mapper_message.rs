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
	#[remain::unsorted]
	#[child]
	KeyDownNoRepeat(Key),
	#[remain::unsorted]
	#[child]
	KeyUpNoRepeat(Key),
	/// The only valid [Key]s for `DoubleClick` are:
	/// - [Key::Lmb]
	/// - [Key::Rmb]
	/// - [Key::Mmb]
	// TODO: Change this from `Key` to `MouseKeys` so the aforementioned valid keys can be enforced
	#[remain::unsorted]
	#[child]
	DoubleClick(Key),

	// Messages
	PointerMove,
	WheelScroll,
}
