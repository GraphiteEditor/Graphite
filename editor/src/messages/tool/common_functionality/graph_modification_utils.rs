use crate::messages::portfolio::document::node_graph;
use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;

use bezier_rs::{ManipulatorGroup, Subpath};
use document_legacy::{LayerId, Operation};
use graph_craft::document::NodeNetwork;
use graphene_core::uuid::ManipulatorGroupId;

use glam::DAffine2;
use std::collections::VecDeque;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<ManipulatorGroupId>>, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	let network = node_graph::new_vector_network(subpaths);
	new_custom_layer(network, layer_path, responses);
}

/// Creates a new custom vector network by resolving node names.
///
/// * `node_names`: The names of the nodes to be resolved and inserted between the path_generator and output nodes.
/// * `subpaths`: The subpath that contains the vector data.
pub fn new_custom_vector_layer(node_names: Vec<String>, subpaths: Vec<Subpath<ManipulatorGroupId>>, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	let network = node_graph::new_custom_vector_network(node_names, subpaths);
	new_custom_layer(network, layer_path, responses);
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
