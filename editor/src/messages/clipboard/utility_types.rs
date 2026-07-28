use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use glam::DAffine2;
use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};
use graph_craft::document::NodeId;
use graphene_std::vector::Vector;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardContentRaw {
	Text(String),
	Svg(String),
	Image { data: Vec<u8>, width: u32, height: u32 },
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardContent {
	Graphite(String),
	Text(String),
	Svg(String),
	Image { data: Vec<u8>, width: u32, height: u32 },
}

pub type ClipboardVectorEntry = (LayerNodeIdentifier, Vector, DAffine2);

/// An entry in the `graphite:` clipboard payload.
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ClipboardItem {
	Layer(ClipboardLayer),
	Nodes(Vec<(NodeId, NodeTemplate)>),
	Vector(Vec<ClipboardVectorEntry>),
	Resource(ClipboardResource),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipboardLayer {
	pub nodes: Vec<(NodeId, NodeTemplate)>,
	pub visible: bool,
	pub locked: bool,
	pub collapsed: bool,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClipboardResource {
	pub id: ResourceId,
	pub hash: Option<ResourceHash>,
	pub sources: Vec<DataSource>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub data: Option<ResourceData>,
}

#[derive(PartialEq, Clone)]
pub struct ResourceData(pub Vec<u8>);
impl std::fmt::Debug for ResourceData {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ResourceData").field("len", &self.0.len()).finish()
	}
}
impl serde::Serialize for ResourceData {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&BASE64.encode(&self.0))
	}
}
impl<'de> serde::Deserialize<'de> for ResourceData {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let encoded = String::deserialize(deserializer)?;
		Ok(Self(BASE64.decode(&encoded).map_err(serde::de::Error::custom)?))
	}
}
