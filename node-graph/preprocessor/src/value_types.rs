//! The compiler's answer to what type a wire carries, for document migrations.

use crate::Preprocessor;
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::TypingContext;
use graph_craft::{Type, item};
use graphene_std::RuntimeHandle;
use graphene_std::platform_application_io::ResourceHash;
use interpreted_executor::node_registry::NODE_REGISTRY;
use std::collections::HashSet;
use std::sync::Arc;

impl Preprocessor {
	/// The value type slot `slot` of `node` carries in `inlined`, a network with no definitions left, by compiling only what feeds that slot.
	/// `None` where that doesn't compile, or where instances of the node in several networks disagree. Costs one compile per instance.
	pub fn input_type(&self, inlined: &NodeNetwork, node: NodeId, slot: usize, resource_hash: &dyn Fn(ResourceId) -> Option<ResourceHash>) -> Option<Type> {
		let instances: Vec<Vec<NodeId>> = inlined.recursive_nodes().filter(|(id, _, _)| **id == node).map(|(_, _, path)| path).collect();

		let mut agreed = None;
		for path in instances {
			let input = inlined.nested_network(&path)?.nodes.get(&node)?.inputs.get(slot)?.clone();
			let value_type = self.type_of_input(inlined, path, input, resource_hash)?;

			if agreed.as_ref().is_some_and(|agreed| *agreed != value_type) {
				return None;
			}
			agreed = Some(value_type);
		}
		agreed
	}

	/// Follows `input` out of networks and through hidden nodes to the output that gives it its type, then compiles that output alone.
	pub fn type_of_input(&self, root: &NodeNetwork, mut path: Vec<NodeId>, mut input: NodeInput, resource_hash: &dyn Fn(ResourceId) -> Option<ResourceHash>) -> Option<Type> {
		let (source, output_index) = loop {
			match input {
				NodeInput::Node { node_id, output_index } => {
					let source = root.nested_network(&path)?.nodes.get(&node_id)?;
					if source.visible {
						break (node_id, output_index);
					}
					input = source.inputs.first()?.clone();
				}
				NodeInput::Import { import_index, .. } => {
					let caller = path.pop()?;
					input = root.nested_network(&path)?.nodes.get(&caller)?.inputs.get(import_index)?.clone();
				}
				_ => return None,
			}
		};

		let mut pruned = root.clone();
		let output_path: Vec<NodeId> = path.iter().copied().chain([source]).collect();
		if !redirect_export_chain(&mut pruned, &output_path, output_index) {
			return None;
		}
		pruned.exports.truncate(1);
		pruned.remove_dead_nodes(0);

		// The compiler asserts every wire resolves, which a stored document doesn't promise
		if !wires_resolve(&pruned) {
			return None;
		}

		// The scopes the runtime injects, given only as types since nothing is evaluated
		let scope_stand_in = |value_type: Type| DocumentNode {
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER),
			inputs: vec![NodeInput::value(TaggedValue::TypeDefault(value_type), false)],
			..Default::default()
		};
		let content = DocumentNode {
			implementation: DocumentNodeImplementation::Network(pruned),
			..Default::default()
		};
		let mut scoped = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [
				(NodeId(0), content),
				(NodeId(1), scope_stand_in(item!(Arc<PlatformEditorApi>))),
				(NodeId(2), scope_stand_in(item!(RuntimeHandle))),
			]
			.into_iter()
			.collect(),
			scope_injections: [
				("editor-api".to_string(), (NodeId(1), item!(Arc<PlatformEditorApi>))),
				("runtime".to_string(), (NodeId(2), item!(RuntimeHandle))),
			]
			.into_iter()
			.collect(),
			definitions: Default::default(),
			generated: true,
		};

		self.preprocess(&mut scoped, resource_hash).ok()?;

		// The compiler asserts every scope input resolves, so content reading any other scope stays unresolved
		if !scope_inputs_resolve(&scoped, &HashSet::new()) {
			return None;
		}

		let proto_network = Compiler {}.compile_single(scoped).ok()?.network;
		let mut typing_context = TypingContext::new(&NODE_REGISTRY);
		typing_context.update(&proto_network).ok()?;
		typing_context.type_of(proto_network.output).map(|types| types.return_value.clone())
	}
}

/// Point the primary export at each level of `network` down through the nodes of `path`, unhiding each so the innermost node's output evaluates.
fn redirect_export_chain(network: &mut NodeNetwork, path: &[NodeId], output_index: usize) -> bool {
	let Some((node_id, deeper_path)) = path.split_first() else { return true };

	let export = NodeInput::node(*node_id, if deeper_path.is_empty() { output_index } else { 0 });
	match network.exports.first_mut() {
		Some(first) => *first = export,
		None => network.exports.push(export),
	}

	let Some(node) = network.nodes.get_mut(node_id) else { return false };
	node.visible = true;

	if deeper_path.is_empty() {
		return true;
	}
	match &mut node.implementation {
		DocumentNodeImplementation::Network(nested) => redirect_export_chain(nested, deeper_path, output_index),
		_ => false,
	}
}

fn wires_resolve(network: &NodeNetwork) -> bool {
	let resolves = |input: &NodeInput| !matches!(input, NodeInput::Node { node_id, .. } if !network.nodes.contains_key(node_id));

	network.exports.iter().all(resolves)
		&& network.nodes.values().all(|node| {
			let nested_resolve = match &node.implementation {
				DocumentNodeImplementation::Network(nested) => wires_resolve(nested),
				_ => true,
			};
			node.inputs.iter().all(resolves) && nested_resolve
		})
}

fn scope_inputs_resolve(network: &NodeNetwork, enclosing_scopes: &HashSet<String>) -> bool {
	let scopes: HashSet<String> = enclosing_scopes.iter().cloned().chain(network.scope_injections.keys().cloned()).collect();

	network.nodes.values().all(|node| {
		let inputs_resolve = node.inputs.iter().all(|input| !matches!(input, NodeInput::Scope(key) if !scopes.contains(key.as_ref())));
		let nested_resolve = match &node.implementation {
			DocumentNodeImplementation::Network(nested) => scope_inputs_resolve(nested, &scopes),
			_ => true,
		};
		inputs_resolve && nested_resolve
	})
}
