use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;

use bezier_rs::{ManipulatorGroup, Subpath};
use document_legacy::{document_metadata::LayerNodeIdentifier, LayerId, Operation};
use graph_craft::document::{value::TaggedValue, DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::uuid::ManipulatorGroupId;

use glam::DAffine2;
use std::collections::VecDeque;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<ManipulatorGroupId>>, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	responses.add(GraphOperationMessage::NewVectorLayer {
		id: *layer_path.last().unwrap(),
		subpaths,
	});
}

pub fn new_custom_layer(network: NodeNetwork, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	responses.add(DocumentMessage::DeselectAllLayers);
	responses.add(Operation::AddFrame {
		path: layer_path.clone(),
		insert_index: -1,
		transform: DAffine2::ZERO.to_cols_array(),
		network,
	});
	responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
}

pub fn set_manipulator_mirror_angle(manipulator_groups: &Vec<ManipulatorGroup<ManipulatorGroupId>>, layer_path: &[u64], mirror_angle: bool, responses: &mut VecDeque<Message>) {
	for manipulator_group in manipulator_groups {
		responses.add(GraphOperationMessage::Vector {
			layer: layer_path.to_owned(),
			modification: VectorDataModification::SetManipulatorHandleMirroring {
				id: manipulator_group.id,
				mirror_angle,
			},
		});
	}
}

/// An immutable reference to a layer within the document node graph for easy access.
pub struct NodeGraphLayer<'a> {
	node_graph: &'a NodeNetwork,
	outwards_links: HashMap<NodeId, Vec<NodeId>>,
	layer_node: NodeId,
}

impl<'a> NodeGraphLayer<'a> {
	/// Get the layer node from the document
	pub fn new(layer: LayerNodeIdentifier, document: &'a document_legacy::document::Document) -> Option<Self> {
		let node_graph = &document.document_network;
		let outwards_links = document.document_network.collect_outwards_links();

		Some(Self {
			node_graph,
			outwards_links,
			layer_node: layer.to_node(),
		})
	}

	/// Get the nearest layer node from the path and the document
	pub fn new_from_path(layer: &[LayerId], document: &'a document_legacy::document::Document) -> Option<Self> {
		let node_graph = &document.document_network;
		let outwards_links = document.document_network.collect_outwards_links();

		let Some(mut layer_node) = layer.last().copied() else {
			error!("Tried to modify root layer");
			return None;
		};
		while node_graph.nodes.get(&layer_node)?.name != "Layer" {
			layer_node = outwards_links.get(&layer_node)?.first().copied()?;
		}
		Some(Self {
			node_graph,
			outwards_links,
			layer_node,
		})
	}

	/// Return an iterator up the primary flow of the layer
	pub fn primary_layer_flow(&self) -> impl Iterator<Item = (&'a DocumentNode, u64)> {
		self.node_graph.primary_flow_from_opt(Some(self.layer_node))
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		for (node, _id) in self.primary_layer_flow() {
			if node.name == node_name {
				let subpaths_input = node.inputs.get(index)?;
				let NodeInput::Value { tagged_value, .. } = subpaths_input else {
					continue;
				};

				return Some(tagged_value);
			}
		}
		None
	}
}
