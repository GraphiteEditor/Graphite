use super::*;

pub(crate) fn add_network() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(i64), 0), NodeInput::import(concrete!(i64), 1)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::structural::ConsNode")),
					..Default::default()
				},
			),
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::AddPairNode")),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

#[test]
fn map_ids() {
	let mut network = add_network();
	network.map_ids(|id| NodeId(id.0 + 1));
	let mapped_add = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(2), 0)],
		nodes: [
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(i64), 0), NodeInput::import(concrete!(i64), 1)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::structural::ConsNode")),
					..Default::default()
				},
			),
			(
				NodeId(2),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(1), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::AddPairNode")),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	};
	assert_eq!(network, mapped_add);
}

#[test]
fn output_source_follows_exports_imports_and_hidden_nodes() {
	let proto_node = |inputs: Vec<NodeInput>| DocumentNode {
		inputs,
		implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::AddPairNode")),
		..Default::default()
	};
	let network_node = |input: NodeInput, network: NodeNetwork| DocumentNode {
		inputs: vec![input],
		implementation: DocumentNodeImplementation::Network(network),
		..Default::default()
	};

	// Network 2 passes its import through to its export, network 3 exports its inner node 0, and node 4 is hidden
	let passing_network = NodeNetwork {
		exports: vec![NodeInput::import(concrete!(i64), 0)],
		..Default::default()
	};
	let wrapping_network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(0), 0)],
		nodes: [(NodeId(0), proto_node(vec![NodeInput::import(concrete!(i64), 0)]))].into_iter().collect(),
		..Default::default()
	};
	let network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(4), 0)],
		nodes: [
			(NodeId(1), proto_node(vec![NodeInput::value(TaggedValue::Integer(1), false)])),
			(NodeId(2), network_node(NodeInput::node(NodeId(1), 0), passing_network)),
			(NodeId(3), network_node(NodeInput::node(NodeId(2), 0), wrapping_network)),
			(
				NodeId(4),
				DocumentNode {
					visible: false,
					..proto_node(vec![NodeInput::node(NodeId(3), 0)])
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	};

	assert_eq!(network.output_source(&[NodeId(1)], 0), Some(vec![NodeId(1)]));
	assert_eq!(
		network.output_source(&[NodeId(2)], 0),
		Some(vec![NodeId(1)]),
		"A network passing its import through gives what's wired to it"
	);
	assert_eq!(network.output_source(&[NodeId(3)], 0), Some(vec![NodeId(3), NodeId(0)]), "A network gives what its export is wired to");
	assert_eq!(network.output_source(&[NodeId(4)], 0), Some(vec![NodeId(3), NodeId(0)]), "A hidden node gives its primary input");
	assert_eq!(network.input_source(&[NodeId(3)], &NodeInput::import(concrete!(i64), 0)), Some(vec![NodeId(1)]));
	assert_eq!(network.output_source(&[NodeId(5)], 0), None);
}

fn node_with(implementation: DocumentNodeImplementation) -> DocumentNode {
	DocumentNode { implementation, ..Default::default() }
}

fn network_with(nodes: impl IntoIterator<Item = (NodeId, DocumentNodeImplementation)>, definitions: impl IntoIterator<Item = (NetworkId, NodeNetwork)>) -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(10), 0)],
		nodes: nodes.into_iter().map(|(id, implementation)| (id, node_with(implementation))).collect(),
		definitions: definitions.into_iter().collect(),
		..Default::default()
	}
}

pub(crate) const ADD: NetworkId = NetworkId(1);

fn two_add_instances() -> NodeNetwork {
	network_with(
		[(NodeId(10), DocumentNodeImplementation::Definition(ADD)), (NodeId(11), DocumentNodeImplementation::Definition(ADD))],
		[(ADD, add_network())],
	)
}

#[test]
fn definitions_inline_into_copies_of_their_network() {
	let mut network = two_add_instances();
	network.inline_definitions().unwrap();

	let inlined = network_with(
		[
			(NodeId(10), DocumentNodeImplementation::Network(add_network())),
			(NodeId(11), DocumentNodeImplementation::Network(add_network())),
		],
		[],
	);
	assert_eq!(network, inlined);
}

#[test]
fn lifted_networks_inline_back_to_the_original() {
	let nested = network_with([(NodeId(10), DocumentNodeImplementation::Network(add_network()))], []);
	let original = network_with(
		[
			(NodeId(10), DocumentNodeImplementation::Network(nested)),
			(NodeId(11), DocumentNodeImplementation::Network(add_network())),
		],
		[],
	);

	let mut network = original.clone();
	let lifted = network.lift_nested_networks();

	assert_eq!(lifted.len(), 3);
	assert_eq!(network.definitions.len(), 3);
	let all_nodes = std::iter::once(&network).chain(network.definitions.values()).flat_map(|network| network.nodes.values());
	assert!(all_nodes.into_iter().all(|node| !matches!(node.implementation, DocumentNodeImplementation::Network(_))));

	let outermost: Vec<_> = lifted.iter().filter(|lifted| lifted.containing.is_none()).map(|lifted| lifted.node_id).collect();
	assert_eq!(outermost.len(), 2);
	assert!(lifted.iter().position(|entry| entry.containing.is_some()) > lifted.iter().position(|entry| entry.containing.is_none()));

	network.inline_definitions().unwrap();
	assert_eq!(network, original);
}

#[test]
fn definitions_referencing_definitions_inline_transitively() {
	let outer = NetworkId(2);
	let mut network = network_with(
		[(NodeId(10), DocumentNodeImplementation::Definition(outer))],
		[(ADD, add_network()), (outer, network_with([(NodeId(10), DocumentNodeImplementation::Definition(ADD))], []))],
	);
	network.inline_definitions().unwrap();

	let inner = network_with([(NodeId(10), DocumentNodeImplementation::Network(add_network()))], []);
	assert_eq!(network, network_with([(NodeId(10), DocumentNodeImplementation::Network(inner))], []));
}

#[test]
fn invalid_definition_tables_are_rejected() {
	let (first, second) = (NetworkId(2), NetworkId(3));
	let mut cyclic = network_with(
		[(NodeId(10), DocumentNodeImplementation::Definition(first))],
		[
			(first, network_with([(NodeId(10), DocumentNodeImplementation::Definition(second))], [])),
			(second, network_with([(NodeId(10), DocumentNodeImplementation::Definition(first))], [])),
		],
	);
	assert_eq!(cyclic.validate_definitions(), Err(DefinitionError::Cycle(vec![first, second])));
	assert_eq!(cyclic.inline_definitions(), Err(DefinitionError::Cycle(vec![first, second])));

	let mut missing = network_with([(NodeId(10), DocumentNodeImplementation::Definition(first))], []);
	assert_eq!(missing.validate_definitions(), Err(DefinitionError::Missing(first)));
	assert_eq!(missing.inline_definitions(), Err(DefinitionError::Missing(first)));

	let mut nested = network_with([(NodeId(10), DocumentNodeImplementation::Network(two_add_instances()))], []);
	assert_eq!(nested.validate_definitions(), Err(DefinitionError::NestedTable));
	assert_eq!(nested.inline_definitions(), Err(DefinitionError::NestedTable));

	assert_eq!(two_add_instances().validate_definitions(), Ok(()));
}

#[test]
fn edits_through_one_instance_reach_every_instance() {
	let mut network = two_add_instances();
	let added = node_with(DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::IdentityNode")));
	network.nested_network_mut(&[NodeId(10)]).unwrap().nodes.insert(NodeId(2), added.clone());

	assert_eq!(network.nested_network(&[NodeId(11)]).unwrap().nodes.get(&NodeId(2)), Some(&added));
	assert_eq!(network.locate(&[NodeId(11)]), Some(Some(ADD)));
	assert_eq!(network.implementation_output_count(&DocumentNodeImplementation::Definition(ADD)), 1);
}

#[test]
fn definition_edits_change_the_network_hash() {
	let mut network = two_add_instances();
	let before = network.current_hash();

	network.definitions.get_mut(&ADD).unwrap().exports.push(NodeInput::node(NodeId(0), 0));
	assert_ne!(network.current_hash(), before);
}

#[test]
fn recursive_nodes_visit_each_definition_once() {
	let network = two_add_instances();
	let body_paths = network.recursive_nodes().map(|(_, _, path)| path).filter(|path| !path.is_empty()).collect::<Vec<_>>();
	assert_eq!(body_paths.len(), add_network().nodes.len());
	assert!(body_paths.windows(2).all(|pair| pair[0] == pair[1]));

	let (first, second) = (NetworkId(2), NetworkId(3));
	let cyclic = network_with(
		[(NodeId(10), DocumentNodeImplementation::Definition(first))],
		[
			(first, network_with([(NodeId(10), DocumentNodeImplementation::Definition(second))], [])),
			(second, network_with([(NodeId(10), DocumentNodeImplementation::Definition(first))], [])),
		],
	);
	assert_eq!(cyclic.recursive_nodes().count(), 3);
}
