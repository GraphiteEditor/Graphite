use crate::messages::portfolio::document::node_graph;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
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

pub fn new_custom_layer(network: NodeNetwork, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	responses.push_back(DocumentMessage::DeselectAllLayers.into());
	responses.push_back(
		Operation::AddNodeGraphFrame {
			path: layer_path.clone(),
			insert_index: -1,
			transform: DAffine2::ZERO.to_cols_array(),
			network,
		}
		.into(),
	);
	responses.add(DocumentMessage::NodeGraphFrameGenerate { layer_path });
}
