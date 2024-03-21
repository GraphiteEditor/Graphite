use crate::messages::prelude::*;

#[impl_message(Message, KeyMapping)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum KeyMappingMessage {
	#[child]
	Lookup(InputMapperMessage),
	#[child]
	ModifyMapping(MappingVariant),
}

#[impl_message(Message, KeyMappingMessage, ModifyMapping)]
#[derive(PartialEq, Eq, Clone, Debug, Default, Hash, serde::Serialize, serde::Deserialize)]
pub enum MappingVariant {
	#[default]
	Default,

	ZoomWithScroll,
}
