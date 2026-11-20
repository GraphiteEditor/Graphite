use crate::document::document_tests::add_network;
use crate::document::value::TaggedValue;
use crate::document::*;
use crate::proto::{ConstructionArgs, ProtoNetwork};
use core_types::{ProtoNodeIdentifier, concrete};
use std::sync::atomic::AtomicU64;

/// Generates IDs from a counter all tests share, so the IDs a test gets depend on what else runs in parallel.
fn gen_node_id() -> NodeId {
	static NODE_ID: AtomicU64 = AtomicU64::new(4);
	NodeId(NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
}

#[test]
fn flatten_add() {
	let mut network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [(
			NodeId(1),
			DocumentNode {
				inputs: vec![NodeInput::import(concrete!(i64), 0), NodeInput::value(TaggedValue::Integer(2), false)],
				implementation: DocumentNodeImplementation::Network(add_network()),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	};
	network.populate_dependants();

	// A counter of its own, since the expected network names the nodes by the IDs it generates
	let next_id = AtomicU64::new(4);
	let gen_id = || NodeId(next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
	network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), gen_id);

	let expected = flatten_add_expected();
	println!("{expected:#?}");
	println!("{network:#?}");

	assert_eq!(expected, network);
}

fn flatten_add_expected() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(11), 0)],
		nodes: [
			(
				NodeId(10),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(i64), 0), NodeInput::node(NodeId(14), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::structural::ConsNode")),
					original_location: OriginalLocation {
						dependants: vec![vec![NodeId(11)]],
						..Default::default()
					},
					..Default::default()
				},
			),
			(
				NodeId(14),
				DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::Integer(2), false)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::value::ClonedNode")),
					original_location: OriginalLocation {
						path: Some(vec![NodeId(4)]),
						dependants: vec![vec![NodeId(10)]],
						..Default::default()
					},
					..Default::default()
				},
			),
			(
				NodeId(11),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(10), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::AddPairNode")),
					original_location: OriginalLocation {
						dependants: vec![vec![]],
						..Default::default()
					},
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

fn two_node_passthrough() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(2), 0)],
		nodes: [
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(i64), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER),
					..Default::default()
				},
			),
			(
				NodeId(2),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(i64), 1)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

fn output_duplicate(network_outputs: Vec<NodeInput>, result_node_input: NodeInput) -> NodeNetwork {
	let mut network = NodeNetwork {
		exports: network_outputs,
		nodes: [
			(
				NodeId(1),
				DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::Number(1.), false), NodeInput::value(TaggedValue::Number(2.), false)],
					implementation: DocumentNodeImplementation::Network(two_node_passthrough()),
					..Default::default()
				},
			),
			(
				NodeId(2),
				DocumentNode {
					inputs: vec![result_node_input],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	};
	let _new_ids = 101..;
	network.populate_dependants();
	network.flatten_with_fns(NodeId(1), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), || NodeId(10000));
	network.validate_dependants();
	network.flatten_with_fns(NodeId(2), |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), || NodeId(10001));
	network.validate_dependants();
	network.remove_dead_nodes(0);
	network.validate_dependants();
	network
}

#[test]
fn simple_duplicate() {
	let result = output_duplicate(vec![NodeInput::node(NodeId(1), 0)], NodeInput::node(NodeId(1), 0));
	println!("{result:#?}");
	result.validate_dependants();
	assert_eq!(result.exports.len(), 1, "The number of outputs should remain as 1");
	assert_eq!(result.exports[0], NodeInput::node(NodeId(11), 0), "The outer network output should be from a duplicated inner network");
	let mut ids = result.nodes.keys().copied().collect::<Vec<_>>();
	ids.sort();
	assert_eq!(ids, vec![NodeId(11), NodeId(10010)], "Should only contain passthrough and values");
}

fn passthrough_network() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(10), 0), NodeInput::node(NodeId(11), 0), NodeInput::node(NodeId(10), 0)],
		nodes: [
			(
				NodeId(1),
				DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(5), 0); 3],
						..Default::default()
					}),
					..Default::default()
				},
			),
			(
				NodeId(10),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(1), 2)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER),
					..Default::default()
				},
			),
			(
				NodeId(11),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(10), 0), NodeInput::node(NodeId(10), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("add")),
					..Default::default()
				},
			),
			(
				NodeId(12),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(10), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("double")),
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
fn remove_passthrough_node() {
	let mut network = passthrough_network();
	network.generate_node_paths(&[]);
	network.populate_dependants();
	network.remove_all_passthrough_nodes();

	assert_eq!(network.nodes.len(), 3);

	let mut generator_dependants = network.nodes.get(&NodeId(1)).unwrap().original_location.dependants[2].clone();
	generator_dependants.sort();
	generator_dependants.dedup();
	assert_eq!(generator_dependants, vec![NodeId(11), NodeId(12)]);

	let add_inputs = &network.nodes.get(&NodeId(11)).unwrap().inputs;
	assert_eq!(add_inputs, &vec![NodeInput::node(NodeId(1), 2); 2]);

	let double_inputs = &network.nodes.get(&NodeId(12)).unwrap().inputs;
	assert_eq!(double_inputs, &vec![NodeInput::node(NodeId(1), 2); 1]);

	assert_eq!(&network.exports, &vec![NodeInput::node(NodeId(1), 2), NodeInput::node(NodeId(11), 0), NodeInput::node(NodeId(1), 2)]);

	network.validate_dependants();
}

/// A hidden node with nothing wired into it (a layer chain's first node, or a generator) feeding a node with several inputs.
fn hidden_source_network(source_inputs: Vec<NodeInput>) -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(2), 0)],
		nodes: [
			(
				NodeId(1),
				DocumentNode {
					inputs: source_inputs,
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("test::Source")),
					visible: false,
					..Default::default()
				},
			),
			(
				NodeId(2),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::value(TaggedValue::Number(2.), false)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("test::Consumer")),
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

// Flattens and strips passthroughs as the compiler does, then resolves the proto nodes, where an unwired input would panic
fn compile_hidden_source(mut network: NodeNetwork) -> ProtoNetwork {
	network.generate_node_paths(&[]);
	network.populate_dependants();
	let ids: Vec<_> = network.nodes.keys().copied().collect();
	ids.into_iter()
		.for_each(|id| network.flatten_with_fns(id, |self_id, inner_id| NodeId(self_id.0 * 10 + inner_id.0), gen_node_id));
	network.remove_all_passthrough_nodes();
	network.into_proto_networks().next().expect("The network should give a proto network")
}

#[test]
fn hidden_node_with_an_unwired_input_passes_its_value() {
	let proto_network = compile_hidden_source(hidden_source_network(vec![NodeInput::value(TaggedValue::Number(1.), true)]));

	// The consumer's first input is now a value node holding the hidden node's unwired value
	let consumer = proto_network.nodes.iter().find(|(id, _)| *id == NodeId(2)).map(|(_, node)| node).expect("The consumer should remain");
	let ConstructionArgs::Nodes(inputs) = &consumer.construction_args else {
		panic!("The consumer should take node inputs")
	};
	let value_node = proto_network
		.nodes
		.iter()
		.find(|(id, _)| *id == inputs[0])
		.map(|(_, node)| node)
		.expect("The consumer's first input should be a node");
	assert!(matches!(&value_node.construction_args, ConstructionArgs::Value(value) if **value == TaggedValue::Number(1.)));
}

#[test]
fn hidden_node_without_inputs_passes_the_unit_value() {
	let proto_network = compile_hidden_source(hidden_source_network(vec![]));

	let consumer = proto_network.nodes.iter().find(|(id, _)| *id == NodeId(2)).map(|(_, node)| node).expect("The consumer should remain");
	let ConstructionArgs::Nodes(inputs) = &consumer.construction_args else {
		panic!("The consumer should take node inputs")
	};
	let value_node = proto_network
		.nodes
		.iter()
		.find(|(id, _)| *id == inputs[0])
		.map(|(_, node)| node)
		.expect("The consumer's first input should be a node");
	assert!(matches!(&value_node.construction_args, ConstructionArgs::Value(value) if **value == TaggedValue::None));
}
