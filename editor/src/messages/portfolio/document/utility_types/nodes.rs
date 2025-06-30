use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;

use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use super::network_interface::NodeNetworkInterface;
use bezier_rs::{ManipulatorGroup, Subpath};
use glam::DVec2;
use graph_craft::document::{NodeId, NodeNetwork};
use graphene_std::vector::PointId;
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
	#[serde(rename = "inSelectedNetwork")]
	pub in_selected_network: bool,
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
	#[serde(rename = "ancestorOfSelected")]
	pub ancestor_of_selected: bool,
	#[serde(rename = "descendantOfSelected")]
	pub descendant_of_selected: bool,
	pub clipped: bool,
	pub clippable: bool,
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

pub fn build_vector_wire(output_position: DVec2, input_position: DVec2, vertical_out: bool, vertical_in: bool, graph_wire_style: GraphWireStyle) -> Subpath<PointId> {
	match graph_wire_style {
		GraphWireStyle::Direct => {
			let horizontal_gap = (output_position.x - input_position.x).abs();
			let vertical_gap = (output_position.y - input_position.y).abs();
			// TODO: Finish this commented out code replacement for the code below it based on this diagram: <https://files.keavon.com/-/SuperbWideFoxterrier/capture.png>
			// // Straight: stacking lines which are always straight, or a straight horizontal wire between two aligned nodes
			// if ((verticalOut && vertical_in) || (!verticalOut && !vertical_in && vertical_gap === 0)) {
			// 	return [
			// 		{ x: output_position.x, y: output_position.y },
			// 		{ x: input_position.x, y: input_position.y },
			// 	];
			// }

			// // L-shape bend
			// if (verticalOut !== vertical_in) {
			// }

			let curve_length = 24.;
			let curve_falloff_rate = curve_length * std::f64::consts::PI * 2.;

			let horizontal_curve_amount = -(2_f64.powf((-10. * horizontal_gap) / curve_falloff_rate)) + 1.;
			let vertical_curve_amount = -(2_f64.powf((-10. * vertical_gap) / curve_falloff_rate)) + 1.;
			let horizontal_curve = horizontal_curve_amount * curve_length;
			let vertical_curve = vertical_curve_amount * curve_length;

			let locations = vec![
				output_position,
				DVec2::new(
					if vertical_out { output_position.x } else { output_position.x + horizontal_curve },
					if vertical_out { output_position.y - vertical_curve } else { output_position.y },
				),
				DVec2::new(
					if vertical_in { input_position.x } else { input_position.x - horizontal_curve },
					if vertical_in { input_position.y + vertical_curve } else { input_position.y },
				),
				DVec2::new(input_position.x, input_position.y),
			];

			let smoothing = 0.5;
			let delta01 = DVec2::new((locations[1].x - locations[0].x) * smoothing, (locations[1].y - locations[0].y) * smoothing);
			let delta23 = DVec2::new((locations[3].x - locations[2].x) * smoothing, (locations[3].y - locations[2].y) * smoothing);

			Subpath::new(
				vec![
					ManipulatorGroup {
						anchor: locations[0],
						in_handle: None,
						out_handle: None,
						id: PointId::generate(),
					},
					ManipulatorGroup {
						anchor: locations[1],
						in_handle: None,
						out_handle: Some(locations[1] + delta01),
						id: PointId::generate(),
					},
					ManipulatorGroup {
						anchor: locations[2],
						in_handle: Some(locations[2] - delta23),
						out_handle: None,
						id: PointId::generate(),
					},
					ManipulatorGroup {
						anchor: locations[3],
						in_handle: None,
						out_handle: None,
						id: PointId::generate(),
					},
				],
				false,
			)
		}
		GraphWireStyle::GridAligned => Subpath::new(Vec::new(), false),
	}
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WirePath {
	// If none, then remove the wire from the map
	#[serde(rename = "pathString")]
	pub path_string: String,
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,
	pub thick: bool,
	pub dashed: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WirePathUpdate {
	pub id: NodeId,
	#[serde(rename = "inputIndex")]
	pub input_index: usize,
	#[serde(rename = "wirePathUpdate")]
	pub wire_path_update: Option<WirePath>,
	// readonly wireSNIUpdate!: number | undefined;
}

#[derive(Copy, Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum GraphWireStyle {
	#[default]
	Direct = 0,
	GridAligned = 1,
}

impl std::fmt::Display for GraphWireStyle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			GraphWireStyle::GridAligned => write!(f, "Grid-Aligned"),
			GraphWireStyle::Direct => write!(f, "Direct"),
		}
	}
}

impl GraphWireStyle {
	pub fn tooltip_description(&self) -> &'static str {
		match self {
			GraphWireStyle::GridAligned => "Wires follow the grid, running in straight lines between nodes",
			GraphWireStyle::Direct => "Wires bend to run at an angle directly between nodes",
		}
	}

	pub fn is_direct(&self) -> bool {
		*self == GraphWireStyle::Direct
	}
}
