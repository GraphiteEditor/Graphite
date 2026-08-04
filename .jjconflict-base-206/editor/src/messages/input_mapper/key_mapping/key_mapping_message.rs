use crate::messages::prelude::*;

#[impl_message(Message, KeyMapping)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum KeyMappingMessage {
	// Sub-messages
	#[child]
	Lookup(InputMapperMessage),

	// Messages
	ModifyMapping {
		mapping: MappingVariant,
	},
}

#[derive(PartialEq, Eq, Clone, Debug, Default, Hash, serde::Serialize, serde::Deserialize)]
pub enum MappingVariant {
	#[default]
	Default,

	ZoomWithScroll,
}
