use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;

use bezier_rs::{ManipulatorGroup, Subpath};
use document_legacy::{document::Document, document_metadata::LayerNodeIdentifier, LayerId, Operation};
use graph_craft::document::{value::TaggedValue, DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::style::{FillType, Gradient};

use glam::DAffine2;
use std::collections::VecDeque;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<ManipulatorGroupId>>, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	responses.add(GraphOperationMessage::NewVectorLayer {
		id: *layer_path.last().unwrap(),
		subpaths,
	});
}

/// Create a legacy node graph frame TODO: remove
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

/// Batch set all of the manipulator groups to mirror on a specific layer
pub fn set_manipulator_mirror_angle(manipulator_groups: &[ManipulatorGroup<ManipulatorGroupId>], layer_path: &[u64], mirror_angle: bool, responses: &mut VecDeque<Message>) {
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

/// Locate the subpaths from the shape nodes of a particular layer
pub fn get_subpaths(layer: LayerNodeIdentifier, document: &Document) -> Option<&Vec<Subpath<ManipulatorGroupId>>> {
	if let TaggedValue::Subpaths(subpaths) = NodeGraphLayer::new(layer, document)?.find_input("Shape", 0)? {
		Some(subpaths)
	} else {
		None
	}
}

/// Get the currently mirrored handles for a particular layer from the shape node
pub fn get_mirror_handles(layer: LayerNodeIdentifier, document: &Document) -> Option<&Vec<ManipulatorGroupId>> {
	if let TaggedValue::ManipulatorGroupIds(mirror_handles) = NodeGraphLayer::new(layer, document)?.find_input("Shape", 1)? {
		Some(mirror_handles)
	} else {
		None
	}
}

/// Get the current gradient of a layer from the closest fill node
pub fn get_gradient(layer: LayerNodeIdentifier, document: &Document) -> Option<Gradient> {
	let inputs = NodeGraphLayer::new(layer, document)?.find_node_inputs("Fill")?;
	let TaggedValue::FillType(FillType::Gradient) = inputs.get(1)?.as_value()? else {
		return None;
	};
	let TaggedValue::GradientType(gradient_type) = inputs.get(3)?.as_value()? else {
		return None;
	};
	let TaggedValue::DVec2(start) = inputs.get(4)?.as_value()? else {
		return None;
	};
	let TaggedValue::DVec2(end) = inputs.get(5)?.as_value()? else {
		return None;
	};
	let TaggedValue::DAffine2(transform) = inputs.get(6)?.as_value()? else {
		return None;
	};
	let TaggedValue::GradientPositions(positions) = inputs.get(7)?.as_value()? else {
		return None;
	};
	Some(Gradient {
		start: start.clone(),
		end: end.clone(),
		transform: transform.clone(),
		positions: positions.clone(),
		gradient_type: gradient_type.clone(),
	})
}

/// Convert subpaths to an iterator of manipulator groups
pub fn get_manipulator_groups(subpaths: &[Subpath<ManipulatorGroupId>]) -> impl Iterator<Item = &bezier_rs::ManipulatorGroup<ManipulatorGroupId>> + DoubleEndedIterator {
	subpaths.iter().flat_map(|subpath| subpath.manipulator_groups())
}

/// Find a manipulator group with a specific id from several subpaths
pub fn get_manipulator_from_id(subpaths: &[Subpath<ManipulatorGroupId>], id: ManipulatorGroupId) -> Option<&bezier_rs::ManipulatorGroup<ManipulatorGroupId>> {
	subpaths.iter().find_map(|subpath| subpath.manipulator_from_id(id))
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

	/// Find all of the inputs of a specific node within the layer's primary flow
	pub fn find_node_inputs(&self, node_name: &str) -> Option<&'a Vec<NodeInput>> {
		for (node, _id) in self.primary_layer_flow() {
			if node.name == node_name {
				return Some(&node.inputs);
			}
		}
		None
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		self.find_node_inputs(node_name)?.get(index)?.as_value()
	}
}
