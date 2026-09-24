use std::collections::BTreeSet;

use core_types::uuid::NodeId as RuntimeNodeId;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};

use crate::{Delta, NoMetadata, Node, NodeId, PeerId, ROOT_NETWORK, Registry, RegistryDelta, Session};

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
					implementation: DocumentNodeImplementation::Network(inner),
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
		nodes: [
			(RuntimeNodeId(1), proto("graphene_core::ops::identity::IdentityNode", vec![NodeInput::node(RuntimeNodeId(2), 0)])),
			(
				RuntimeNodeId(2),
				DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::Integer(7), true)],
					implementation: DocumentNodeImplementation::Network(group),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

fn find_node<'a>(network: &'a NodeNetwork, path: &[RuntimeNodeId], id: RuntimeNodeId) -> &'a DocumentNode {
	let nested = network.nested_network(path).expect("the path should resolve");
	nested.nodes.get(&id).expect("the node should exist")
}

/// A node projected on its own must be what the whole-document conversion holds for it, metadata
/// included, since a mirror patched from projections has to match a mirror rebuilt whole.
#[test]
fn projecting_a_node_matches_the_whole_conversion() {
	let resources = graphene_resource::ResourceRegistry::new();
	let conversion = Registry::convert_from_runtime(&nested_network(), &NoMetadata, &resources, PeerId(1)).expect("conversion");
	let registry = &conversion.registry;
	let (whole, node_entries, network_entries) = registry.to_runtime_with_full_metadata(&conversion.declarations).expect("whole conversion");

	let projection = registry.runtime_projection(&conversion.declarations);
	for &id in registry.node_instances.keys() {
		let projected = projection.node(id).expect("every stored node projects");
		let (path, local_id) = projection.node_address(id).expect("every stored node has an address");
		assert_eq!((path.as_slice(), local_id), (projected.network_path.as_slice(), projected.local_id));

		assert_eq!(
			&projected.node,
			find_node(&whole, &path, local_id),
			"node {id} projected differently than the whole conversion converts it"
		);

		let own_entry = node_entries.iter().find(|entry| entry.storage_id == id).expect("the whole conversion emits an entry per node");
		assert_eq!(projected.node_entries.first(), Some(own_entry), "the node's own entry comes first");

		// The nested entries are exactly the whole conversion's entries under this node.
		let nested_prefix: Vec<RuntimeNodeId> = [path.as_slice(), &[local_id]].concat();
		let expected_nested: BTreeSet<_> = node_entries
			.iter()
			.filter(|entry| entry.network_path.starts_with(&nested_prefix))
			.map(|entry| entry.storage_id)
			.collect();
		let projected_nested: BTreeSet<_> = projected.node_entries.iter().skip(1).map(|entry| entry.storage_id).collect();
		assert_eq!(projected_nested, expected_nested, "nested node entries under {id}");

		let expected_networks: BTreeSet<_> = network_entries
			.iter()
			.filter(|entry| entry.network_path.starts_with(&nested_prefix))
			.map(|entry| entry.network_id)
			.collect();
		let projected_networks: BTreeSet<_> = projected.network_entries.iter().map(|entry| entry.network_id).collect();
		assert_eq!(projected_networks, expected_networks, "nested network entries under {id}");
	}
}

#[test]
fn a_network_entry_projects_its_dense_exports_and_path() {
	let resources = graphene_resource::ResourceRegistry::new();
	let conversion = Registry::convert_from_runtime(&nested_network(), &NoMetadata, &resources, PeerId(1)).expect("conversion");
	let registry = &conversion.registry;
	let (whole, _, _) = registry.to_runtime_with_full_metadata(&conversion.declarations).expect("whole conversion");
	let projection = registry.runtime_projection(&conversion.declarations);

	let root = projection.network_entry(ROOT_NETWORK).expect("root");
	assert!(root.network_path.is_empty());
	assert_eq!(root.exports, whole.exports);

	for &network_id in registry.networks.keys() {
		let entry = projection.network_entry(network_id).expect("every stored network projects");
		let nested = whole.nested_network(&entry.network_path).expect("the projected path exists in the whole conversion");
		assert_eq!(entry.exports, nested.exports, "exports of the network at {:?}", entry.network_path);
		assert_eq!(entry.scope_injections, nested.scope_injections);
		if let Some(owner) = projection.owner(network_id) {
			let (owner_path, owner_local) = projection.node_address(owner).expect("owner address");
			assert_eq!(
				entry.network_path,
				[owner_path.as_slice(), &[owner_local]].concat(),
				"a network's path ends in the node implementing it"
			);
		}
	}
}

/// A network no node implements is not in the runtime, so nothing inside it has an address.
#[test]
fn an_unreachable_network_has_no_address() {
	let mut registry = Registry::default();
	registry.networks.insert(crate::NetworkId(5), crate::Network::default());
	let mut node = Node::dummy();
	node.network = crate::NetworkId(5);
	registry.node_instances.insert(NodeId(9), node);

	let declarations = crate::Declarations::new();
	let projection = registry.runtime_projection(&declarations);
	assert_eq!(projection.network_path(crate::NetworkId(5)), None);
	assert_eq!(projection.node_address(NodeId(9)), None);
	assert!(matches!(projection.node(NodeId(9)), Err(crate::to_runtime::ConversionError::NetworkNotFound(_))));
}

/// The counter tells a mirror that a refold produced values the applied ops do not account for. A refold
/// that lands on the same values, which is the common case when a hot op retires, leaves it alone.
#[test]
fn rederivations_count_only_refolds_that_change_the_working_registry() {
	let mut session = Session::with_peer(PeerId(1));
	session
		.stage_computed_ops(vec![RegistryDelta::AddNetwork {
			id: crate::NetworkId(3),
			network: crate::Network::default(),
		}])
		.expect("stage");
	let last = session.hot_log().last().expect("staged").timestamp;
	session.retire(last).expect("retire");
	assert_eq!(session.working_rederivations(), 0);

	// A refold over the same history and an empty hot log reproduces the same values.
	session.document.refold_owed = true;
	session.merge(Vec::<Delta>::new()).expect("a merge absorbing nothing settles the owed refold");
	assert_eq!(session.working_rederivations(), 0, "a refold to the same values is not a rederivation");

	// A removal held hot and an addition arriving retired do not commute: applied in arrival order the
	// node is present, refolded it is absent. That is what a mirror following the ops cannot know.
	let node_id = NodeId(4);
	let mut node = Node::dummy();
	node.network = crate::NetworkId(3);
	session
		.stage_computed_ops(vec![RegistryDelta::RemoveNode { id: node_id, snapshot: node.clone() }])
		.expect("a removal of an absent node stages as a no-op");
	let mut other = session.clone();
	other.document.hot_log.clear();
	other.commit_op_for_test(RegistryDelta::AddNode { id: node_id, node }).expect("the other peer adds the node");
	let incoming: Vec<Delta> = other.cloned_deltas().into_iter().filter(|delta| session.delta(delta.id).is_none()).collect();

	session.merge(incoming).expect("merge");
	assert!(session.registry().node_instances.contains_key(&node_id), "applied in arrival order the addition lands last");

	session.document.refold_owed = true;
	session.merge(Vec::<Delta>::new()).expect("refold");
	assert!(!session.registry().node_instances.contains_key(&node_id), "refolded, the hot removal lands last");
	assert_eq!(session.working_rederivations(), 1, "a refold that changed the values is a rederivation");
}
