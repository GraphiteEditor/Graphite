use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceHash;
use std::sync::Arc;

#[impl_message(Message, ResourceStorage)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResourceStorageMessage {
	Store { data: Arc<[u8]> },
	GarbageCollect { used: Box<[ResourceHash]> },
}
