use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use super::network_interface::NodeNetworkInterface;

use graph_craft::document::{NodeId, NodeNetwork};

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct LayerPanelEntry {
	pub id: NodeId,
	pub alias: String,
	pub tooltip: String,
	#[serde(rename = "childrenAllowed")]
	pub children_allowed: bool,
	#[serde(rename = "childrenPresent")]
	pub children_present: bool,
	pub expanded: bool,
	pub depth: usize,
	pub visible: bool,
	#[serde(rename = "parentsVisible")]
	pub parents_visible: bool,
	pub unlocked: bool,
	#[serde(rename = "parentsUnlocked")]
	pub parents_unlocked: bool,
	#[serde(rename = "parentId")]
	pub parent_id: Option<NodeId>,
	pub selected: bool,
	#[serde(rename = "inSelectedNetwork")]
	pub in_selected_network: bool,
	#[serde(rename = "selectedParent")]
	pub selected_parent: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct SelectedNodes(pub Vec<NodeId>);

impl SelectedNodes {
	pub fn layer_visible(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> bool {
		layer.ancestors(network_interface.document_metadata()).all(|layer| {
			if layer != LayerNodeIdentifier::ROOT_PARENT {
				network_interface.is_visible(&layer.to_node(), &[])
			} else {
				true
			}
		})
	}

	pub fn selected_visible_layers<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| self.layer_visible(layer, network_interface))
	}

	pub fn layer_locked(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> bool {
		layer.ancestors(network_interface.document_metadata()).any(|layer| {
			if layer != LayerNodeIdentifier::ROOT_PARENT {
				network_interface.is_locked(&layer.to_node(), &[])
			} else {
				false
			}
		})
	}

	pub fn selected_unlocked_layers<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| !self.layer_locked(layer, network_interface))
	}

	pub fn selected_visible_and_unlocked_layers<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| self.layer_visible(layer, network_interface) && !self.layer_locked(layer, network_interface))
	}

	pub fn selected_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		metadata.all_layers().filter(|layer| self.0.contains(&layer.to_node()))
	}

	pub fn selected_layers_except_artboards<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| !network_interface.is_artboard(&layer.to_node(), &[]))
	}

	pub fn selected_layers_contains(&self, layer: LayerNodeIdentifier, metadata: &DocumentMetadata) -> bool {
		self.selected_layers(metadata).any(|selected| selected == layer)
	}

	pub fn selected_nodes(&self) -> impl Iterator<Item = &NodeId> + '_ {
		self.0.iter()
	}

	pub fn selected_nodes_ref(&self) -> &Vec<NodeId> {
		&self.0
	}

	pub fn network_has_selected_nodes(&self, network: &NodeNetwork) -> bool {
		self.0.iter().any(|node_id| network.nodes.contains_key(node_id))
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

	pub fn add_selected_nodes(&mut self, new: Vec<NodeId>) {
		self.0.extend(new);
	}

	pub fn clear_selected_nodes(&mut self) {
		self.0 = Vec::new();
	}

	pub fn replace_with(&mut self, new: Vec<NodeId>) -> Vec<NodeId> {
		std::mem::replace(&mut self.0, new)
	}

	pub fn filtered_selected_nodes(&self, node_ids: std::collections::HashSet<NodeId>) -> SelectedNodes {
		SelectedNodes(self.0.iter().filter(|node_id| node_ids.contains(node_id)).cloned().collect())
	}
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct CollapsedLayers(pub Vec<LayerNodeIdentifier>);
