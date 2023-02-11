use crate::messages::input_mapper::utility_types::{input_keyboard::Key, misc::MappingEntry};
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, InputMapper)]
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

	// TODO(multisn8): this is really unclean, perhaps the actual input messages should go in a
	// subenum?
	#[remain::unsorted]
	DeleteMapping(Box<MappingEntry>),
	#[remain::unsorted]
	CreateMapping(Box<MappingEntry>),
}
