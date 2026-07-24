mod caches;
#[cfg(test)]
mod characterization_tests;
mod deserialization;
mod hit_tests;
mod layout;
mod memo_network;
mod mutations;
mod queries;
mod resolved_types;
pub mod storage_metadata;
mod structure;
mod types;
#[cfg(test)]
mod validation;

pub use types::*;

use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier, NodeRelations};
use super::misc::PTZ;
use super::nodes::SelectedNodes;
use crate::consts::{
	EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP, EXPORTS_TO_TOP_EDGE_PIXEL_GAP, GRID_SIZE, HALF_GRID_SIZE, IMPORTS_TO_LEFT_EDGE_PIXEL_GAP, IMPORTS_TO_TOP_EDGE_PIXEL_GAP, LAYER_INDENT_OFFSET, NODE_CHAIN_WIDTH,
	STACK_VERTICAL_GAP,
};
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type};
use crate::messages::portfolio::document::node_graph::utility_types::{Direction, FrontendClickTargets, FrontendGraphDataType, FrontendGraphInput, FrontendGraphOutput};
use crate::messages::portfolio::document::overlays::utility_functions::text_width;
use crate::messages::portfolio::document::utility_types::network_interface::resolved_types::ResolvedDocumentNodeTypes;
use crate::messages::portfolio::document::utility_types::wires::{GraphWireStyle, WirePath, WirePathUpdate, build_thick_wire_center_line, build_vector_wire};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::NumberInputMode;
use deserialization::deserialize_node_persistent_metadata;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::Type;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, OldDocumentNodeImplementation, OldNodeNetwork};
use graphene_std::ContextDependencies;
use graphene_std::Graphic;
use graphene_std::list::List;
use graphene_std::math::quad::Quad;
use graphene_std::subpath::Subpath;
use graphene_std::transform::Footprint;
use graphene_std::vector::click_target::{ClickTarget, ClickTargetType, FreePoint};
use graphene_std::vector::{PointId, Vector, VectorModificationType};
use kurbo::BezPath;
use memo_network::MemoNetwork;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

/// All network modifications should be done through this API, so the fields cannot be public. However, all fields within this struct can be public since it it not possible to have a public mutable reference.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkInterface {
	/// The node graph that generates this document's artwork. It recursively stores its sub-graphs, so this root graph is the whole snapshot of the document content.
	/// A public mutable reference should never be created. It should only be mutated through custom setters which perform the necessary side effects to keep network_metadata in sync
	network: MemoNetwork,
	/// Stores all editor information for a NodeNetwork. Should automatically kept in sync by the setter methods when changes to the document network are made.
	network_metadata: NodeNetworkMetadata,
	// TODO: Wrap in TransientMetadata Option
	/// Stores the document network's structural topology. Should automatically kept in sync by the setter methods when changes to the document network are made.
	#[serde(skip)]
	document_metadata: DocumentMetadata,
	/// All input/output types based on the compiled network.
	#[serde(skip)]
	pub resolved_types: ResolvedDocumentNodeTypes,
	#[serde(skip)]
	transaction_status: TransactionStatus,
}

impl Clone for NodeNetworkInterface {
	fn clone(&self) -> Self {
		Self {
			network: self.network.clone(),
			network_metadata: self.network_metadata.clone(),
			document_metadata: Default::default(),
			resolved_types: Default::default(),
			transaction_status: TransactionStatus::Finished,
		}
	}
}

impl PartialEq for NodeNetworkInterface {
	fn eq(&self, other: &Self) -> bool {
		self.network == other.network && self.network_metadata == other.network_metadata
	}
}

impl NodeNetworkInterface {
	/// Add DocumentNodePath input to the PathModifyNode protonode
	pub fn migrate_path_modify_node(&mut self) {
		fix_network(self.document_network_mut());
		fn fix_network(network: &mut NodeNetwork) {
			for node in network.nodes.values_mut() {
				if let Some(network) = node.implementation.get_network_mut() {
					fix_network(network);
				}
				if let DocumentNodeImplementation::ProtoNode(protonode) = &node.implementation
					&& protonode.as_str().contains("PathModifyNode")
					&& node.inputs.len() < 3
				{
					node.inputs.push(NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath));
				}
			}
		}
	}
}

#[cfg(test)]
mod network_interface_tests {
	use crate::test_utils::test_prelude::*;
	#[tokio::test]
	async fn copy_isolated_node() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let rectangle = editor
			.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER))
			.await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![rectangle] }).await;
		let frontend_messages = editor.handle_message(NodeGraphMessage::Copy).await;
		let clipboard = frontend_messages
			.into_iter()
			.find_map(|msg| match msg {
				FrontendMessage::TriggerClipboardWrite { content } => Some(content),
				_ => None,
			})
			.expect("copy message should be dispatched");
		println!("Clipboard: {clipboard}");
		editor
			.handle_message(ClipboardMessage::ReadClipboard {
				content: ClipboardContentRaw::Text(clipboard),
			})
			.await;
		let nodes = &mut editor.active_document_mut().network_interface.network_mut(&[]).unwrap().nodes;
		let orignal = nodes.remove(&rectangle).expect("original node should exist");
		assert!(
			nodes.values().any(|other| *other == orignal),
			"duplicated node should exist\nother nodes: {nodes:#?}\norignal {orignal:#?}"
		);
	}
}
