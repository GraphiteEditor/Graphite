use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceId;
use graphene_std::text::Font;
use std::sync::Arc;

#[impl_message(Message, DocumentMessage, Resource)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResourceMessage {
	StoreEmbedded { resource_id: ResourceId, data: Arc<[u8]> },
	AddFont { resource_id: ResourceId, font: Font },
	Resolve,
	ResolveStep { resource_id: ResourceId },
	Resolved { resource_id: ResourceId, data: Arc<[u8]> },
}
