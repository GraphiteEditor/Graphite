use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier};

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
	pub name: String,
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
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct SelectedNodes(pub Vec<NodeId>);

impl SelectedNodes {
	pub fn layer_visible(&self, layer: LayerNodeIdentifier, metadata: &DocumentMetadata) -> bool {
		layer.ancestors(metadata).all(|layer| {
			if layer != LayerNodeIdentifier::ROOT_PARENT {
				metadata.node_is_visible(layer.to_node())
			} else {
				true
			}
		})
	}

	pub fn selected_visible_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(metadata).filter(move |&layer| self.layer_visible(layer, metadata))
	}

	pub fn layer_locked(&self, layer: LayerNodeIdentifier, metadata: &DocumentMetadata) -> bool {
		layer.ancestors(metadata).any(|layer| {
			if layer != LayerNodeIdentifier::ROOT_PARENT {
				metadata.node_is_locked(layer.to_node())
			} else {
				false
			}
		})
	}

	pub fn selected_unlocked_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(metadata).filter(move |&layer| !self.layer_locked(layer, metadata))
	}

	pub fn selected_visible_and_unlocked_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.selected_layers(metadata)
			.filter(move |&layer| self.layer_visible(layer, metadata) && !self.layer_locked(layer, metadata))
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

	// All selected nodes must be in the same network
	pub fn selected_nodes<'a>(&'a self, network: &'a NodeNetwork) -> impl Iterator<Item = &NodeId> + '_ {
		self.0
			.iter()
			.filter(|node_id| network.nodes.contains_key(*node_id) || **node_id == network.imports_metadata.0 || **node_id == network.exports_metadata.0)
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

	// TODO: This function is run when a node in the layer panel is currently selected, and a new node is selected in the graph, as well as when a node is currently selected in the graph and a node in the layer panel is selected. These are fundamentally different operations, since different nodes should be selected in each case, but cannot be distinguished. Currently it is not possible to shift+click a node in the node graph while a layer is selected. Instead of set_selected_nodes, add_selected_nodes should be used.
	pub fn set_selected_nodes(&mut self, new: Vec<NodeId>, document_network: &NodeNetwork, network_path: &[NodeId]) {
		let Some(network) = document_network.nested_network(network_path) else { return };

		let mut new_nodes = new;

		// If any nodes to add are in the document network, clear selected nodes in the current network
		if new_nodes.iter().any(|node_to_add| document_network.nodes.contains_key(node_to_add)) {
			new_nodes.retain(|selected_node| {
				document_network.nodes.contains_key(selected_node) || document_network.imports_metadata.0 == *selected_node || document_network.exports_metadata.0 == *selected_node
			});
		}
		// If not, then clear any nodes that are not in the current network
		else {
			new_nodes.retain(|selected_node| network.nodes.contains_key(selected_node) || network.imports_metadata.0 == *selected_node || network.exports_metadata.0 == *selected_node);
		}

		self.0 = new_nodes;
	}

	pub fn add_selected_nodes(&mut self, new: Vec<NodeId>, document_network: &NodeNetwork, network_path: &[NodeId]) {
		let Some(network) = document_network.nested_network(network_path) else { return };

		// If the nodes to add are in the document network, clear selected nodes in the current network
		if new.iter().any(|node_to_add| document_network.nodes.contains_key(node_to_add)) {
			self.retain_selected_nodes(|selected_node| {
				document_network.nodes.contains_key(selected_node) || document_network.imports_metadata.0 == *selected_node || document_network.exports_metadata.0 == *selected_node
			});
		} else {
			self.retain_selected_nodes(|selected_node| network.nodes.contains_key(selected_node) || network.imports_metadata.0 == *selected_node || network.exports_metadata.0 == *selected_node);
		}

		self.0.extend(new);
	}

	pub fn clear_selected_nodes(&mut self) {
		self.0 = Vec::new();
	}
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
pub struct CollapsedLayers(pub Vec<LayerNodeIdentifier>);
