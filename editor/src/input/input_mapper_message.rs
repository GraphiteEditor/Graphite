use super::keyboard::Key;
use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, InputMapper)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum InputMapperMessage {
	#[child]
	KeyDown(Key),
	#[child]
	KeyUp(Key),
	MouseScroll,
	PointerMove,
}
