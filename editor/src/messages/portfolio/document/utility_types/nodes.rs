use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use super::network_interface::NodeNetworkInterface;
use crate::messages::frontend::IconName;
use crate::messages::tool::common_functionality::graph_modification_utils;
use glam::DVec2;
use graph_craft::document::{NodeId, NodeNetwork};

/// Represents an entry in the layer tree hierarchy, sent to the frontend.
/// Each entry contains its layer ID and a list of its visible children.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LayerStructureEntry {
	/// The node ID of the layer this entry represents.
	#[serde(rename = "layerId")]
	pub layer_id: NodeId,
	/// The expanded child entries nested within this layer. Empty when the layer is collapsed or has no children.
	pub children: Vec<LayerStructureEntry>,
	/// Whether this layer has children reachable in the graph, even when they are omitted from `children` because the layer is collapsed.
	#[serde(rename = "childrenPresent")]
	pub children_present: bool,
	/// Whether any descendant layer in the graph is selected, including through collapsed subtrees not listed in `children`.
	#[serde(rename = "descendantSelected")]
	pub descendant_selected: bool,
	pub selected: bool,
	/// Whether the layer itself is visible, which the frontend passes down to its children's rows as inherited visibility.
	pub visible: bool,
	/// Whether the layer itself is unlocked, which the frontend passes down to its children's rows as an inherited lock.
	pub unlocked: bool,
	/// The rows listing the layer's chain nodes beneath its own row, one per node while its chain is expanded and otherwise none.
	#[serde(rename = "chainRows")]
	pub chain_rows: u32,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LayerPanelEntry {
	pub id: NodeId,
	#[serde(rename = "implementationName")]
	pub implementation_name: String,
	#[serde(rename = "iconName")]
	pub icon_name: IconName,
	/// The nodes in the chain feeding this layer, in the graph's left-to-right order.
	#[serde(rename = "chainNodes")]
	pub chain_nodes: Vec<LayerPanelChainNode>,
	/// Whether the chain is expanded into a row per node beneath the layer's row.
	#[serde(rename = "chainExpanded")]
	pub chain_expanded: bool,
	/// How the active tool edits the layer node itself, if it does, for the tooltip of its outlined icon.
	#[serde(rename = "toolTargetDescription")]
	pub tool_target_description: Option<String>,
	/// Whether an edit in progress, like a drag, has written to the layer node itself, which brightens its icon's outline.
	#[serde(rename = "editInProgress")]
	pub edit_in_progress: bool,
	/// The node the active tool would add to the chain to edit this layer, shown ghosted where it would go.
	#[serde(rename = "toolTargetInsertion")]
	pub tool_target_insertion: Option<LayerPanelChainInsertion>,
	pub alias: String,
	#[serde(rename = "inSelectedNetwork")]
	pub in_selected_network: bool,
	#[serde(rename = "childrenAllowed")]
	pub children_allowed: bool,
	pub visible: bool,
	pub unlocked: bool,
	pub selected: bool,
	#[serde(rename = "descendantOfSelected")]
	pub descendant_of_selected: bool,
	pub clipped: bool,
	pub clippable: bool,
}

/// A node in the chain feeding a layer, shown as a button with its icon in the layer's row.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LayerPanelChainNode {
	pub id: NodeId,
	pub icon: IconName,
	pub name: String,
	/// The name shown in place of a custom one, as the placeholder while renaming.
	#[serde(rename = "implementationName")]
	pub implementation_name: String,
	/// Whether the node is selected, shown in every layer whose chain shares it.
	pub selected: bool,
	/// Whether the node can be dragged to reorder it, unlike a source node (with no primary input) fixed at the chain's upstream end.
	pub reorderable: bool,
	pub visible: bool,
	/// How the active tool edits this node, if it does, for the tooltip of its outlined icon.
	#[serde(rename = "toolTargetDescription")]
	pub tool_target_description: Option<String>,
	/// Whether an edit in progress, like a drag, has written to this node, which brightens its outline.
	#[serde(rename = "editInProgress")]
	pub edit_in_progress: bool,
}

/// A node not yet in a layer's chain, which the active tool would add there.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LayerPanelChainInsertion {
	/// The position among the chain nodes, counted from the upstream end.
	pub index: u32,
	pub icon: IconName,
	pub name: String,
	pub description: String,
}

/// IMPORTANT: the same node may appear multiple times.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SelectedNodes(pub Vec<NodeId>);

impl SelectedNodes {
	pub fn layer_visible(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> bool {
		network_interface.is_layer_visible(layer)
	}

	pub fn selected_visible_layers<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
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

	pub fn selected_unlocked_layers<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| !self.layer_locked(layer, network_interface))
	}

	pub fn selected_visible_and_unlocked_layers<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| self.layer_visible(layer, network_interface) && !self.layer_locked(layer, network_interface))
	}

	pub fn selected_visible_and_unlocked_layers_mean_average_origin<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> DVec2 {
		let (sum, count) = self
			.selected_visible_and_unlocked_layers(network_interface)
			.map(|layer| graph_modification_utils::get_viewport_origin(layer, network_interface))
			.fold((glam::DVec2::ZERO, 0), |(sum, count), item| (sum + item, count + 1));
		if count == 0 { DVec2::ZERO } else { sum / count as f64 }
	}

	pub fn selected_visible_and_unlocked_median_points<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> DVec2 {
		let (sum, count) = self
			.selected_visible_and_unlocked_layers(network_interface)
			.map(|layer| graph_modification_utils::get_viewport_center(layer, network_interface))
			.fold((glam::DVec2::ZERO, 0), |(sum, count), item| (sum + item, count + 1));
		if count == 0 { DVec2::ZERO } else { sum / count as f64 }
	}

	pub fn selected_layers<'a>(&'a self, metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		metadata.all_layers().filter(|layer| self.0.contains(&layer.to_node()))
	}

	pub fn selected_layers_except_artboards<'a>(&'a self, network_interface: &'a NodeNetworkInterface) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		self.selected_layers(network_interface.document_metadata())
			.filter(move |&layer| !network_interface.is_artboard(&layer.to_node(), &[]))
	}

	pub fn selected_layers_contains(&self, layer: LayerNodeIdentifier, metadata: &DocumentMetadata) -> bool {
		self.selected_layers(metadata).any(|selected| selected == layer)
	}

	/// IMPORTANT: the same node may appear multiple times.
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

	pub fn filtered_selected_nodes(&self, filter: impl Fn(&NodeId) -> bool) -> SelectedNodes {
		SelectedNodes(self.0.iter().copied().filter(filter).collect())
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
/// Tracks which layer occurrences are collapsed in the Layers panel. Each entry is a "tree path":
/// the sequence of ancestor node IDs from the root down to the collapsed layer. This allows the same
/// layer appearing under multiple parents to have independent expand/collapse state per occurrence.
pub struct CollapsedLayers(pub Vec<Vec<NodeId>>);
