use crate::messages::prelude::*;
use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};
use graphene_std::text::Font;
use std::sync::Arc;

#[impl_message(Message, DocumentMessage, Resource)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResourceMessage {
	StoreEmbedded { resource_id: ResourceId, data: Arc<[u8]> },
	AddFont { resource_id: ResourceId, font: Font },
	ResolveAll,
	Resolve { resource_id: ResourceId },
	Resolved { resource_id: ResourceId, source: DataSource, hash: ResourceHash },
	ResolveFailed { resource_id: ResourceId },
}
