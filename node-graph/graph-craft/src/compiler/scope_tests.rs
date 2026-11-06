use crate::document::document_tests::{ADD, add_network};
use crate::document::value::TaggedValue;
use crate::document::*;
use core_types::{ProtoNodeIdentifier, Type, concrete};

/// Two instances of `ADD`, one of them through another entry, with a scope input inside the entry it names.
fn shared_instances_with_a_scope_input() -> NodeNetwork {
	let outer = NetworkId(2);
	let mut add = add_network();
	add.nodes.get_mut(&NodeId(1)).unwrap().inputs.push(NodeInput::scope("injected"));
	let node = |implementation, inputs| DocumentNode {
		implementation,
		inputs,
		..Default::default()
	};
	let value = |number| NodeInput::value(TaggedValue::Integer(number), false);

	let outer_network = NodeNetwork {
		exports: vec![NodeInput::node(NodeId(10), 0)],
		nodes: [(NodeId(10), node(DocumentNodeImplementation::Definition(ADD), vec![NodeInput::import(concrete!(i64), 0), value(2)]))]
			.into_iter()
			.collect(),
		..Default::default()
	};
	let injected = node(DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::IdentityNode")), vec![value(7)]);

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(11), 0)],
		nodes: [
			(NodeId(10), node(DocumentNodeImplementation::Definition(outer), vec![value(1)])),
			(NodeId(11), node(DocumentNodeImplementation::Definition(ADD), vec![NodeInput::node(NodeId(10), 0), value(3)])),
			(NodeId(20), injected),
		]
		.into_iter()
		.collect(),
		scope_injections: [("injected".to_string(), (NodeId(20), concrete!(i64)))].into_iter().collect(),
		definitions: [(ADD, add), (outer, outer_network)].into_iter().collect(),
		..Default::default()
	}
}

/// A root instance of `ADD` and one inside `group`, which shadows the root's `injected` with a node of type `shadowing_type`.
fn shadowed_scope_input(shadowing_type: Type) -> NodeNetwork {
	let group = NetworkId(2);
	let mut add = add_network();
	add.nodes.get_mut(&NodeId(1)).unwrap().inputs.push(NodeInput::scope("injected"));
	let node = |implementation| DocumentNode { implementation, ..Default::default() };
	let identity = || node(DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::ops::IdentityNode")));

	let group_network = NodeNetwork {
		nodes: [(NodeId(10), node(DocumentNodeImplementation::Definition(ADD))), (NodeId(21), identity())].into_iter().collect(),
		scope_injections: [("injected".to_string(), (NodeId(21), shadowing_type))].into_iter().collect(),
		..Default::default()
	};
	NodeNetwork {
		nodes: [
			(NodeId(10), node(DocumentNodeImplementation::Definition(group))),
			(NodeId(11), node(DocumentNodeImplementation::Definition(ADD))),
			(NodeId(20), identity()),
		]
		.into_iter()
		.collect(),
		scope_injections: [("injected".to_string(), (NodeId(20), concrete!(i64)))].into_iter().collect(),
		definitions: [(ADD, add), (group, group_network)].into_iter().collect(),
		..Default::default()
	}
}

fn scope_import_types(network: &NodeNetwork) -> Vec<Type> {
	let mut types: Vec<Type> = network
		.definitions
		.values()
		.filter_map(|definition| match definition.nodes.get(&NodeId(1))?.inputs.last()? {
			NodeInput::Import { import_type, .. } => Some(import_type.clone()),
			_ => None,
		})
		.collect();
	types.sort_by_key(|ty| format!("{ty:?}"));
	types
}

#[test]
fn the_nearest_injection_resolves_a_scope_input() {
	let mut network = shadowed_scope_input(concrete!(i64));
	network.resolve_scope_inputs();

	assert_eq!(scope_import_types(&network), vec![concrete!(i64)], "a shadowing injection of the same type shares the entry");
	assert_eq!(network.nodes[&NodeId(11)].inputs.last(), Some(&NodeInput::node(NodeId(20), 0)));
	assert_eq!(network.definitions[&NetworkId(2)].nodes[&NodeId(10)].inputs.last(), Some(&NodeInput::node(NodeId(21), 0)));
}

#[test]
fn a_shadowing_injection_of_another_type_specializes_the_entry() {
	let mut network = shadowed_scope_input(concrete!(f64));
	network.resolve_scope_inputs();

	assert_eq!(scope_import_types(&network), vec![concrete!(i64), concrete!(f64)]);
	let DocumentNodeImplementation::Definition(root_instance) = network.nodes[&NodeId(11)].implementation else {
		panic!()
	};
	let DocumentNodeImplementation::Definition(group_instance) = network.definitions[&NetworkId(2)].nodes[&NodeId(10)].implementation else {
		panic!()
	};
	assert_ne!(root_instance, group_instance);
	assert_eq!(network.definitions[&NetworkId(2)].nodes[&NodeId(10)].inputs.last(), Some(&NodeInput::node(NodeId(21), 0)));
}

#[test]
fn a_table_compiles_like_its_inlined_network() {
	let table = shared_instances_with_a_scope_input();
	let mut inlined = table.clone();
	inlined.inline_definitions().unwrap();

	// Flattening gives value nodes random IDs, which their paths carry, so only the other paths are compared
	let compile = |network| {
		let mut compiled = crate::graphene_compiler::Compiler {}.compile_single(network).expect("the network compiles").network;
		let mut paths: Vec<_> = compiled
			.nodes
			.iter()
			.filter(|(_, node)| node.identifier != ProtoNodeIdentifier::new("core_types::value::ClonedNode"))
			.map(|(_, node)| node.original_location.path.clone())
			.collect();
		paths.sort();
		compiled.nodes.iter_mut().for_each(|(_, node)| node.original_location = OriginalLocation::default());
		(compiled, paths)
	};
	assert_eq!(compile(table), compile(inlined));
}
