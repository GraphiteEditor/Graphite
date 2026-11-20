use crate::document::value::TaggedValue;
use crate::document::*;
use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
use core_types::{Context, ProtoNodeIdentifier, concrete};

#[test]
fn resolve_proto_node_add() {
	let document_node = DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0)],
		call_argument: concrete!(i64),
		implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::structural::ConsNode")),
		..Default::default()
	};

	let proto_node = document_node.resolve_proto_node();
	let reference = ProtoNode {
		identifier: ProtoNodeIdentifier::new("core_types::structural::ConsNode"),
		call_argument: concrete!(i64),
		construction_args: ConstructionArgs::Nodes(vec![NodeId(0)]),
		..Default::default()
	};
	assert_eq!(proto_node, reference);
}

#[test]
fn resolve_flatten_add_as_proto_network() {
	let construction_network = ProtoNetwork {
		inputs: Vec::new(),
		output: NodeId(11),
		nodes: [
			(
				NodeId(10),
				ProtoNode {
					identifier: ProtoNodeIdentifier::new("core_types::structural::ConsNode"),
					call_argument: concrete!(i64),
					construction_args: ConstructionArgs::Nodes(vec![NodeId(14)]),
					original_location: OriginalLocation {
						path: Some(vec![NodeId(1), NodeId(0)]),
						dependants: vec![vec![NodeId(11)]],
						..Default::default()
					},

					..Default::default()
				},
			),
			(
				NodeId(11),
				ProtoNode {
					identifier: ProtoNodeIdentifier::new("core_types::ops::AddPairNode"),
					call_argument: concrete!(Context),
					construction_args: ConstructionArgs::Nodes(vec![NodeId(10)]),
					original_location: OriginalLocation {
						path: Some(vec![NodeId(1), NodeId(1)]),
						dependants: vec![vec![]],
						..Default::default()
					},
					..Default::default()
				},
			),
			(
				NodeId(14),
				ProtoNode {
					identifier: ProtoNodeIdentifier::new("core_types::value::ClonedNode"),
					call_argument: concrete!(core_types::Context),
					construction_args: ConstructionArgs::Value(TaggedValue::Integer(2).into()),
					original_location: OriginalLocation {
						path: Some(vec![NodeId(1), NodeId(4)]),
						dependants: vec![vec![NodeId(10)]],
						..Default::default()
					},
					..Default::default()
				},
			),
		]
		.into_iter()
		.collect(),
	};
	let mut network = flat_network();
	network.populate_dependants();
	let mut resolved_network = network.into_proto_networks().collect::<Vec<_>>();
	resolved_network[0].nodes.sort_unstable_by_key(|(id, _)| *id);

	println!("{:#?}", resolved_network[0]);
	println!("{construction_network:#?}");
	pretty_assertions::assert_eq!(resolved_network[0], construction_network);
}

fn flat_network() -> NodeNetwork {
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(11), 0)],
		nodes: [
			(
				NodeId(10),
				DocumentNode {
					inputs: vec![NodeInput::node(NodeId(14), 0)],
					call_argument: concrete!(i64),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::structural::ConsNode")),
					original_location: OriginalLocation {
						path: Some(vec![NodeId(1), NodeId(0)]),
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
						path: Some(vec![NodeId(1), NodeId(4)]),
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
						path: Some(vec![NodeId(1), NodeId(1)]),
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
