use crate::messages::portfolio::document::node_graph;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use document_legacy::{LayerId, Operation};
use glam::DAffine2;
use graphene_core::uuid::ManipulatorGroupId;
use std::collections::VecDeque;

type LayerPath = Vec<LayerId>;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<ManipulatorGroupId>>, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	responses.push_back(DocumentMessage::DeselectAllLayers.into());
	let network = node_graph::new_vector_network(subpaths);

	responses.push_back(
		Operation::AddNodeGraphFrame {
			path: layer_path,
			insert_index: -1,
			transform: DAffine2::ZERO.to_cols_array(),
			network,
		}
		.into(),
	);
	responses.push_back(DocumentMessage::NodeGraphFrameGenerate.into());
}

pub fn set_property(layer_path: Vec<LayerId>) {}
