mod apply;
mod caches;
#[cfg(test)]
mod characterization_tests;
mod deserialization;
pub mod editor_delta;
#[cfg(test)]
mod editor_delta_tests;
mod frontend;
mod geometry;
mod hit_tests;
mod invalidation;
mod layout;
mod memo_network;
mod mutations;
mod queries;
mod resolved_types;
pub mod storage_metadata;
mod store;
mod structure;
mod template;
mod types;
#[cfg(test)]
mod validation;
mod view;

pub use editor_delta::{EditorDelta, NetworkMetadataChange, NodeMetadataChange};
use store::Guarded;
pub use store::NodeLocator;
pub use template::*;
pub use types::*;
pub use view::{NetworkError, NetworkView};

use super::document_metadata::{DocumentMetadata, LayerNodeIdentifier, NodeRelations};
use super::misc::PTZ;
use super::nodes::SelectedNodes;
use crate::consts::{
	EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP, EXPORTS_TO_TOP_EDGE_PIXEL_GAP, GRID_SIZE, HALF_GRID_SIZE, IMPORTS_TO_LEFT_EDGE_PIXEL_GAP, IMPORTS_TO_TOP_EDGE_PIXEL_GAP, LAYER_INDENT_OFFSET, NODE_CHAIN_WIDTH,
	STACK_VERTICAL_GAP,
};
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type, resolve_proto_node_type};
use crate::messages::portfolio::document::node_graph::utility_types::{Direction, FrontendClickTargets, FrontendGraphDataType, FrontendGraphInput, FrontendGraphOutput};
use crate::messages::portfolio::document::utility_types::network_interface::resolved_types::ResolvedDocumentNodeTypes;
use crate::messages::portfolio::document::utility_types::text_metrics::text_width;
use crate::messages::portfolio::document::utility_types::wires::{GraphWireStyle, WirePath, WirePathUpdate, build_thick_wire_center_line, build_vector_wire};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::NumberInputMode;
use deserialization::deserialize_node_persistent_metadata;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::Type;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, LiftedNetwork, NetworkId, NetworkLocation, NodeId, NodeInput, NodeNetwork, OldDocumentNodeImplementation, OldNodeNetwork};
use graphene_std::Appearance;
use graphene_std::math::quad::Quad;
use graphene_std::registry::MULTI_OUTPUT_NODES;
use graphene_std::transform::Footprint;
use graphene_std::vector::click_target::{ClickTarget, ClickTargetType, FreePoint};
use graphene_std::vector::{Vector, VectorModificationType};
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
	network: Guarded<MemoNetwork>,
	/// Stores all editor information for a NodeNetwork. Should automatically kept in sync by the setter methods when changes to the document network are made.
	network_metadata: Guarded<NodeNetworkMetadata>,
	// TODO: Wrap in a TransientCache
	/// Stores the document network's structural topology. Should automatically kept in sync by the setter methods when changes to the document network are made.
	#[serde(skip)]
	document_metadata: DocumentMetadata,
	/// All input/output types based on the compiled network.
	#[serde(skip)]
	pub resolved_types: ResolvedDocumentNodeTypes,
	#[serde(skip)]
	transaction_status: TransactionStatus,
	/// What the writes since the last drain changed, in write order, appended by `store.rs` alone.
	///
	/// Emitting here rather than in the mutators makes it impossible to write without saying what was
	/// written, the same way writing through the store makes it impossible to update one tree without
	/// the other. Transient: a snapshot clone starts empty, since the snapshot is a state rather than
	/// a set of changes.
	#[serde(skip)]
	deltas: Vec<EditorDelta>,
	/// The document network nodes the open transaction has written to, read from `deltas`, for the Layers panel to highlight during an edit like a drag.
	#[serde(skip)]
	transaction_edited_nodes: HashSet<NodeId>,
	/// How far into `deltas` the reading for `transaction_edited_nodes` has reached.
	#[serde(skip)]
	transaction_edited_nodes_scanned: usize,
	/// Whether `transaction_edited_nodes` changed since `update_transaction_edited_nodes` last reported it.
	#[serde(skip)]
	transaction_edited_nodes_changed: bool,
	/// The redirect dots in each network that an edit took a wire from, and on which side, since `remove_orphaned_dots` last judged them there.
	/// Only these are judged, so a Passthrough left without a wire by anything other than an edit (like one a migration made from a removed node) is left in place.
	#[serde(skip)]
	dots_that_lost_a_wire: HashMap<Vec<NodeId>, HashSet<(NodeId, DotSide)>>,
}

impl Clone for NodeNetworkInterface {
	fn clone(&self) -> Self {
		Self {
			network: self.network.clone(),
			network_metadata: self.network_metadata.clone(),
			document_metadata: Default::default(),
			resolved_types: Default::default(),
			transaction_status: TransactionStatus::Finished,
			deltas: Vec::new(),
			transaction_edited_nodes: HashSet::new(),
			transaction_edited_nodes_scanned: 0,
			transaction_edited_nodes_changed: false,
			dots_that_lost_a_wire: HashMap::new(),
		}
	}
}

impl PartialEq for NodeNetworkInterface {
	fn eq(&self, other: &Self) -> bool {
		self.network == other.network && self.network_metadata == other.network_metadata
	}
}

impl NodeNetworkInterface {
	// TODO: Eventually remove this document upgrade code
	/// Moves the interface metadata an older document stored on each instance node onto the network implementing it.
	pub fn move_interface_metadata_into_networks(&mut self) {
		self.migrate_metadata(NodeNetworkMetadata::move_interface_metadata_into_networks);
	}

	// TODO: Eventually remove this document upgrade code
	/// Normalizes the stored types of every node at every nesting level, for an older document whose stored types predate the current form.
	pub fn normalize_stored_types(&mut self) {
		self.migrate_graph(NodeNetwork::normalize_stored_types);
	}

	// TODO: Eventually remove this document upgrade code
	/// Add DocumentNodePath input to the PathModifyNode protonode
	pub fn migrate_path_modify_node(&mut self) {
		self.migrate_graph(fix_network);
		fn fix_network(network: &mut NodeNetwork) {
			for definition in network.definitions.values_mut() {
				fix_network(definition);
			}
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
		let nodes = &editor.active_document().network_interface.document_network().nodes;
		let original = nodes.get(&rectangle).expect("original node should exist");
		assert!(
			nodes.iter().any(|(node_id, other)| *node_id != rectangle && other == original),
			"duplicated node should exist\nother nodes: {nodes:#?}\noriginal {original:#?}"
		);
	}

	#[test]
	fn instance_paths_resolve_through_the_definitions_table() {
		use super::*;

		let definition = NetworkId(1);
		let instance = |implementation| DocumentNode { implementation, ..Default::default() };
		let instances = [NodeId(10), NodeId(11)];

		let network = NodeNetwork {
			nodes: instances.iter().map(|&id| (id, instance(DocumentNodeImplementation::Definition(definition)))).collect(),
			definitions: [(definition, NodeNetwork::value_network(instance(DocumentNodeImplementation::default())))].into_iter().collect(),
			..Default::default()
		};

		let mut definition_metadata = NodeNetworkMetadata::default();
		definition_metadata.persistent_metadata.reference = Some("Artboard".into());
		definition_metadata.persistent_metadata.node_metadata.insert(NodeId(0), DocumentNodeMetadata::default());
		definition_metadata.persistent_metadata.export_names = vec![String::new()];

		let mut network_metadata = NodeNetworkMetadata::default();
		for id in instances {
			network_metadata.persistent_metadata.node_metadata.insert(id, DocumentNodeMetadata::default());
		}
		network_metadata.persistent_metadata.definitions.insert(definition, definition_metadata);

		let interface = NodeNetworkInterface::from_trees(network.clone(), network_metadata.clone());

		let paths = instances.map(|id| [id]);
		let views = paths.each_ref().map(|path| interface.view(path).unwrap());
		assert!(std::ptr::eq(views[0].network, views[1].network));
		assert!(std::ptr::eq(views[0].metadata, views[1].metadata));
		assert!(views[0].node_metadata(&NodeId(0)).is_ok());

		let document_view = interface.view(&[]).unwrap();
		assert_eq!(document_view.reference(&NodeId(10)), Ok(Some(DefinitionIdentifier::Network("Artboard".into()))));
		assert_eq!(document_view.number_of_outputs(&NodeId(10)), Ok(1));
		assert_eq!(interface.validate_invariants(), Vec::<String>::new());

		let unused = NetworkId(2);
		let mut network_with_unused = network.clone();
		network_with_unused
			.definitions
			.insert(unused, NodeNetwork::value_network(instance(DocumentNodeImplementation::default())));
		let mut metadata_with_unused = network_metadata.clone();
		metadata_with_unused.persistent_metadata.definitions.insert(unused, NodeNetworkMetadata::default());
		let with_unused = NodeNetworkInterface::from_trees(network_with_unused, metadata_with_unused);
		assert_eq!(with_unused.validate_invariants(), vec!["Node 0 in network [] has no metadata entry".to_string()]);

		let mut mismatched_network = network;
		mismatched_network.nodes.get_mut(&NodeId(11)).unwrap().inputs.push(NodeInput::value(TaggedValue::None, false));
		let mismatched = NodeNetworkInterface::from_trees(mismatched_network, network_metadata);
		assert!(mismatched.validate_invariants().iter().any(|violation| violation.contains("another instance of definition")));
	}

	#[test]
	fn metadata_walks_reach_every_instance_of_a_definition() {
		use super::*;

		let definition = NetworkId(1);
		let instance = |implementation| DocumentNode { implementation, ..Default::default() };
		let instances = [NodeId(10), NodeId(11)];

		let network = NodeNetwork {
			nodes: instances.iter().map(|&id| (id, instance(DocumentNodeImplementation::Definition(definition)))).collect(),
			definitions: [(definition, NodeNetwork::value_network(instance(DocumentNodeImplementation::default())))].into_iter().collect(),
			..Default::default()
		};

		let mut layer_metadata = DocumentNodeMetadata::default();
		layer_metadata.persistent_metadata.node_type_metadata = NodeTypePersistentMetadata::layer(IVec2::ZERO);
		let mut definition_metadata = NodeNetworkMetadata::default();
		definition_metadata.persistent_metadata.node_metadata.insert(NodeId(0), layer_metadata);

		let mut network_metadata = NodeNetworkMetadata::default();
		for id in instances {
			network_metadata.persistent_metadata.node_metadata.insert(id, DocumentNodeMetadata::default());
		}
		network_metadata.persistent_metadata.definitions.insert(definition, definition_metadata);

		let mut interface = NodeNetworkInterface::from_trees(network, network_metadata);

		let mut layer_paths = interface.layer_paths();
		layer_paths.sort();
		assert_eq!(layer_paths, vec![vec![NodeId(10), NodeId(0)], vec![NodeId(11), NodeId(0)]]);

		interface.set_reference(&NodeId(10), &[], Some("Artboard".into()));
		assert!(interface.is_artboard(&NodeId(11), &[]));
	}

	#[test]
	fn interface_names_live_on_the_implementing_network() {
		use super::*;

		let definition = NetworkId(1);
		let instance = |implementation| DocumentNode {
			implementation,
			inputs: vec![NodeInput::value(TaggedValue::None, false)],
			..Default::default()
		};
		let instances = [NodeId(10), NodeId(11)];

		let network = NodeNetwork {
			nodes: instances.iter().map(|&id| (id, instance(DocumentNodeImplementation::Definition(definition)))).collect(),
			definitions: [(definition, NodeNetwork::value_network(DocumentNode::default()))].into_iter().collect(),
			..Default::default()
		};

		let mut definition_metadata = NodeNetworkMetadata::default();
		definition_metadata.persistent_metadata.node_metadata.insert(NodeId(0), DocumentNodeMetadata::default());

		let mut instance_metadata = DocumentNodeMetadata::default();
		instance_metadata.persistent_metadata.input_metadata = vec![InputMetadata::default()];
		instance_metadata.persistent_metadata.output_names = vec![String::new()];
		let mut network_metadata = NodeNetworkMetadata::default();
		for id in instances {
			network_metadata.persistent_metadata.node_metadata.insert(id, instance_metadata.clone());
		}
		network_metadata.persistent_metadata.definitions.insert(definition, definition_metadata);

		let mut interface = NodeNetworkInterface::from_trees(network, network_metadata);
		assert_eq!(interface.validate_invariants(), Vec::<String>::new());

		interface.set_import_export_name("Shared".into(), ImportOrExport::Import(0), &[NodeId(10)]);
		interface.set_import_export_name("Result".into(), ImportOrExport::Export(0), &[NodeId(10)]);

		let view = interface.view(&[]).unwrap();
		assert_eq!(view.persistent_input_metadata(&NodeId(11), 0).unwrap().input_name, "Shared");
		assert_eq!(view.output_names(&NodeId(11)).unwrap(), ["Result".to_string()]);
	}

	#[test]
	fn interface_edits_through_one_instance_reach_every_instance() {
		use super::*;

		let definition = NetworkId(1);
		let instances = [NodeId(10), NodeId(11)];
		let network = NodeNetwork {
			nodes: instances
				.iter()
				.map(|&id| {
					let node = DocumentNode {
						implementation: DocumentNodeImplementation::Definition(definition),
						..Default::default()
					};
					(id, node)
				})
				.collect(),
			definitions: [(definition, NodeNetwork::value_network(DocumentNode::default()))].into_iter().collect(),
			..Default::default()
		};

		let mut definition_metadata = NodeNetworkMetadata::default();
		definition_metadata.persistent_metadata.node_metadata.insert(NodeId(0), DocumentNodeMetadata::default());
		definition_metadata.persistent_metadata.export_names = vec![String::new()];
		let mut network_metadata = NodeNetworkMetadata::default();
		for id in instances {
			network_metadata.persistent_metadata.node_metadata.insert(id, DocumentNodeMetadata::default());
		}
		network_metadata.persistent_metadata.definitions.insert(definition, definition_metadata);

		let mut interface = NodeNetworkInterface::from_trees(network, network_metadata);
		let through_first = [NodeId(10)];

		interface.add_import(TaggedValue::Integer(1), true, -1, "Amount", "", &through_first);
		interface.add_import(TaggedValue::Integer(2), true, -1, "Scale", "", &through_first);
		assert_eq!(instances.map(|id| interface.number_of_inputs(&id, &[])), [2, 2]);
		assert_eq!(interface.validate_invariants(), Vec::<String>::new());

		interface.reorder_import(1, 0, &through_first);
		let view = interface.view(&[]).unwrap();
		assert_eq!(view.persistent_input_metadata(&NodeId(11), 0).unwrap().input_name, "Scale");
		assert_eq!(view.input(&InputConnector::node_at_index(NodeId(11), 0)).unwrap().as_value(), Some(&TaggedValue::Integer(2)));

		interface.remove_import(0, &through_first);
		interface.add_export(TaggedValue::None, -1, "Extra", &through_first);
		assert_eq!(instances.map(|id| interface.number_of_inputs(&id, &[])), [1, 1]);
		assert_eq!(instances.map(|id| interface.number_of_outputs(&id, &[])), [2, 2]);
		assert_eq!(interface.validate_invariants(), Vec::<String>::new());
	}
}
