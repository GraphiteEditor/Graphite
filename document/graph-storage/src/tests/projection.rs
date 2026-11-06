use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkId as RuntimeNetworkId, NetworkLocation, NodeInput, NodeNetwork};

use crate::to_runtime::ConversionError;
use crate::{Declarations, Network, NetworkId, NoMetadata, Node, NodeId, PeerId, ROOT_NETWORK, Registry};

fn proto(identifier: &'static str, inputs: Vec<NodeInput>) -> DocumentNode {
	DocumentNode {
		inputs,
		implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
		..Default::default()
	}
}

/// A root network holding a proto node and a group, with another proto node and a nested group inside
/// the group, so projection is exercised two networks deep.
fn nested_network() -> NodeNetwork {
	const INNER: RuntimeNetworkId = RuntimeNetworkId(30);
	const GROUP: RuntimeNetworkId = RuntimeNetworkId(20);

	let inner = NodeNetwork {
		exports: vec![NodeInput::node(RuntimeNodeId(30), 0)],
		nodes: [(RuntimeNodeId(30), proto("graphene_core::ops::identity::IdentityNode", vec![NodeInput::import(concrete!(u32), 0)]))]
			.into_iter()
			.collect(),
		..Default::default()
	};
	let group = NodeNetwork {
		exports: vec![NodeInput::node(RuntimeNodeId(20), 0)],
		nodes: [
			(RuntimeNodeId(20), proto("graphene_core::ops::identity::IdentityNode", vec![NodeInput::node(RuntimeNodeId(21), 0)])),
			(
				RuntimeNodeId(21),
				DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::Integer(3), false)],
					implementation: DocumentNodeImplementation::Definition(INNER),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	};
	NodeNetwork {
		exports: vec![NodeInput::node(RuntimeNodeId(1), 0)],
		definitions: [(INNER, inner), (GROUP, group)].into_iter().collect(),
		nodes: [
			(RuntimeNodeId(1), proto("graphene_core::ops::identity::IdentityNode", vec![NodeInput::node(RuntimeNodeId(2), 0)])),
			(
				RuntimeNodeId(2),
				DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::Integer(7), true)],
					implementation: DocumentNodeImplementation::Definition(GROUP),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

/// A node projected alone must match the whole-document conversion, metadata included, since a mirror
/// patched from projections has to match one rebuilt whole.
#[test]
fn projecting_a_node_matches_the_whole_conversion() {
	let resources = graphene_resource::ResourceRegistry::new();
	let conversion = Registry::convert_from_runtime(&nested_network(), &NoMetadata, &resources, PeerId(1)).expect("conversion");
	let registry = &conversion.registry;
	let (whole, node_entries, _) = registry.to_runtime_with_full_metadata(&conversion.declarations).expect("whole conversion");

	let projection = registry.runtime_projection(&conversion.declarations);
	for &id in registry.node_instances.keys() {
		let projected = projection.node(id).expect("every stored node projects");
		let (network, local_id) = projection.node_address(id).expect("every stored node has an address");
		assert_eq!((&network, local_id), (&projected.network, projected.local_id));

		assert_eq!(
			whole.located_network(network).and_then(|network| network.nodes.get(&local_id)),
			Some(&projected.node),
			"node {id} projected differently than the whole conversion converts it"
		);

		let own_entry = node_entries.iter().find(|entry| entry.storage_id == id).expect("the whole conversion emits an entry per node");
		assert_eq!(&projected.metadata, own_entry);
	}
}

#[test]
fn a_network_entry_projects_its_dense_exports_and_location() {
	let resources = graphene_resource::ResourceRegistry::new();
	let conversion = Registry::convert_from_runtime(&nested_network(), &NoMetadata, &resources, PeerId(1)).expect("conversion");
	let registry = &conversion.registry;
	let (whole, _, _) = registry.to_runtime_with_full_metadata(&conversion.declarations).expect("whole conversion");
	let projection = registry.runtime_projection(&conversion.declarations);

	let root = projection.network_entry(ROOT_NETWORK).expect("root");
	assert_eq!(root.network, NetworkLocation::default());
	assert_eq!(root.exports, whole.exports);

	for &network_id in registry.networks.keys() {
		let entry = projection.network_entry(network_id).expect("every stored network projects");
		let located = whole.located_network(entry.network).expect("the projected location exists in the whole conversion");
		assert_eq!(entry.exports, located.exports, "exports of the network at {:?}", entry.network);
		assert_eq!(entry.scope_injections, located.scope_injections);
	}
}

/// A network no node implements is not in the runtime, so nothing inside it has an address.
#[test]
fn an_unreachable_network_has_no_address() {
	let mut registry = Registry::default();
	registry.networks.insert(NetworkId(5), Network::default());
	let mut node = Node::dummy();
	node.network = NetworkId(5);
	registry.node_instances.insert(NodeId(9), node);

	let declarations = Declarations::new();
	let projection = registry.runtime_projection(&declarations);
	assert_eq!(projection.network_location(NetworkId(5)), None);
	assert_eq!(projection.node_address(NodeId(9)), None);
	assert!(matches!(projection.node(NodeId(9)), Err(ConversionError::NetworkNotFound(_))));
}

/// Nodes that implement the same network all name one table entry.
#[test]
fn a_network_several_nodes_implement_has_one_location() {
	let mut registry = Registry::default();
	registry.networks.insert(ROOT_NETWORK, Network::default());
	registry.networks.insert(NetworkId(5), Network::default());
	for id in [1, 2] {
		let mut node = Node::dummy();
		node.network = ROOT_NETWORK;
		node.implementation = crate::Implementation::Network(NetworkId(5));
		registry.node_instances.insert(NodeId(id), node);
	}

	let declarations = Declarations::new();
	let projection = registry.runtime_projection(&declarations);
	assert_eq!(projection.network_location(NetworkId(5)), Some(Some(RuntimeNetworkId(5))));
	assert_eq!(projection.network_location(ROOT_NETWORK), Some(NetworkLocation::default()));
}
