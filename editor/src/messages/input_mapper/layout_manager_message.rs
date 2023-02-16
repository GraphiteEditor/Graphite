use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, LayoutManager)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum LayoutManagerMessage {
	#[child]
	Lookup(InputMapperMessage),
	#[child]
	ModifyLayout(MappingLayout),
}

#[remain::sorted]
#[impl_message(Message, LayoutManagerMessage, ModifyLayout)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum MappingLayout {
	Blender,
	Default,
}
