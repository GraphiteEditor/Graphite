use core_types::uuid::NodeId;
use glam::{DAffine2, DVec2};
use graphene_hash::CacheHash;
use graphic_types::Graphic;
use network_connector::NetworkConnectorKind;
use scene::GraphUiScene;
use wire::WireInProgress;

pub mod card;
mod dashes;
pub mod dot;
pub mod error_badge;
pub mod icons;
pub mod layer;
mod look;
pub mod metrics;
pub mod network_connector;
pub mod nodes;
pub mod render;
pub mod replicated_map;
pub mod scene;
#[cfg(test)]
mod test_rows;
mod text;
pub mod wire;

#[derive(CacheHash)]
pub struct GraphUi {
	pub scene: GraphUiScene,
	pub config: GraphUiConfig,
	pub thumbnails: Vec<(NodeId, Graphic)>,
}

#[derive(Clone, Copy, Debug, CacheHash)]
pub struct GraphUiConfig {
	pub view: DAffine2,
	pub fade_artwork: f64,
	pub wire_in_progress: Option<WireInProgress>,
	pub reorder_bar: Option<(NetworkConnectorKind, DVec2)>,
	pub box_selection: Option<[DVec2; 2]>,
}
