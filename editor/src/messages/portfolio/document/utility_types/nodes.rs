use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier};

use graph_craft::document::NodeId;

use serde::ser::SerializeStruct;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct RawBuffer(Vec<u8>);

impl From<&[u64]> for RawBuffer {
	fn from(iter: &[u64]) -> Self {
		let v_from_raw: Vec<u8> = iter.iter().flat_map(|x| x.to_ne_bytes()).collect();
		Self(v_from_raw)
	}
}
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct JsRawBuffer(Vec<u8>);

impl From<RawBuffer> for JsRawBuffer {
	fn from(buffer: RawBuffer) -> Self {
		Self(buffer.0)
	}
}
impl serde::Serialize for JsRawBuffer {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let mut buffer = serializer.serialize_struct("Buffer", 2)?;
		buffer.serialize_field("pointer", &(self.0.as_ptr() as usize))?;
		buffer.serialize_field("length", &(self.0.len()))?;
		buffer.end()
	}
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub enum LayerClassification {
	#[default]
	Folder,
	Artboard,
	Layer,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct LayerPanelEntry {
	pub id: NodeId,
	pub name: String,
	pub tooltip: String,
	#[serde(rename = "layerClassification")]
	pub layer_classification: LayerClassification,
	pub expanded: bool,
	pub visible: bool,
	#[serde(rename = "parentId")]
	pub parent_id: Option<NodeId>,
	pub depth: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct SelectedNodes(pub Vec<NodeId>);

impl SelectedNodes {
	pub fn layer_visible(&self, layer: LayerNodeIdentifier, metadata: &DocumentMetadata) -> bool {
		layer.ancestors(metadata).all(|layer| metadata.node_is_visible(layer.to_node()))
	}

	pub fn selected_visible_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(metadata).filter(move |&layer| self.layer_visible(layer, metadata))
	}

	pub fn layer_ulocked(&self, layer: LayerNodeIdentifier, network: &NodeNetwork, metadata: &DocumentMetadata) -> bool {
		!layer.ancestors(metadata).any(|layer| network.locked.contains(&layer.to_node()))
	}

	pub fn selected_ulocked_layers<'a>(&'a self, network: &'a NodeNetwork, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(metadata).filter(move |&layer| self.layer_ulocked(layer, network, metadata))
	}

	pub fn selected_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		metadata.all_layers().filter(|layer| self.0.contains(&layer.to_node()))
	}

	pub fn selected_layers_except_artboards<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(metadata).filter(move |&layer| !metadata.is_artboard(layer))
	}

	pub fn selected_layers_contains(&self, layer: LayerNodeIdentifier, metadata: &DocumentMetadata) -> bool {
		self.selected_layers(metadata).any(|selected| selected == layer)
	}

	pub fn selected_nodes(&self) -> core::slice::Iter<'_, NodeId> {
		self.0.iter()
	}

	pub fn selected_nodes_ref(&self) -> &Vec<NodeId> {
		&self.0
	}

	pub fn has_selected_nodes(&self) -> bool {
		!self.0.is_empty()
	}

	pub fn retain_selected_nodes(&mut self, f: impl FnMut(&NodeId) -> bool) {
		self.0.retain(f);
	}

	pub fn set_selected_nodes(&mut self, new: Vec<NodeId>) {
		self.0 = new;
	}

	pub fn add_selected_nodes(&mut self, iter: impl IntoIterator<Item = NodeId>) {
		self.0.extend(iter);
	}

	pub fn clear_selected_nodes(&mut self) {
		self.set_selected_nodes(Vec::new());
	}
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct CollapsedLayers(pub Vec<LayerNodeIdentifier>);
