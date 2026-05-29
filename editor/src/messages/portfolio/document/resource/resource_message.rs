use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceId;
use std::sync::Arc;

#[impl_message(Message, DocumentMessage, Resource)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResourceMessage {
	StoreEmbedded { resource_id: ResourceId, data: Arc<[u8]> },
}
