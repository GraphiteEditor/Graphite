use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, KeyMapping)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum KeyMappingMessage {
	#[child]
	Lookup(InputMapperMessage),
	#[child]
	ModifyMapping(MappingVariant),
}

#[impl_message(Message, KeyMappingMessage, ModifyMapping)]
#[derive(PartialEq, Eq, Clone, Debug, Default, Hash, Serialize, Deserialize)]
pub enum MappingVariant {
	#[default]
	Default,

	ZoomWithScroll,
}
