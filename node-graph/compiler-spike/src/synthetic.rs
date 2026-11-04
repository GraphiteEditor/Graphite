//! Synthetic workloads in the exported schema, built from real registry nodes so the baseline type checks them.

use crate::workload::{Implementation, Import, Input, Network, Node, NodeReference, Type, Value, load_network};

/// The ID `wrap_network_in_scope` gives the document network inside the editor's scope wrapper, followed by the render network.
const DOCUMENT_NODE: u64 = 0;
const RENDER_NODE: u64 = 1;

/// Places a document in the editor's scope wrapper, taken from an exported demo, as `wrap_network_in_scope` does.
/// Without `render`, the document is exported directly, for documents whose output is not a graphic.
pub fn wrap_in_scope(mut document: Network, render: bool) -> Network {
	let mut scoped = load_network("procedural-string-lights");
	scoped.definitions = std::mem::take(&mut document.definitions);
	let (_, document_node) = scoped
		.nodes
		.iter_mut()
		.find(|(id, _)| *id == DOCUMENT_NODE)
		.expect("the scope wrapper should contain the document node");
	document_node.implementation = Implementation::Network(document);

	if !render {
		scoped.nodes.retain(|(id, _)| *id != RENDER_NODE);
		scoped.exports = vec![Input::Node(NodeReference { node: DOCUMENT_NODE, output: 0 })];
	}
	scoped
}

const ADD: &str = "math_nodes::AddNode";
const ANIMATION_TIME: &str = "graphene_core::animation::AnimationTimeNode";
const CONTEXT: &str = "core::option::Option<alloc::sync::Arc<core_types::context::OwnedContextImpl>>";

fn number() -> Type {
	Type::Item(Box::new(Type::Concrete("f64".to_string())))
}

fn context() -> Type {
	Type::Concrete(CONTEXT.to_string())
}

/// Accumulates the nodes of one network, handing out sequential IDs.
#[derive(Default)]
struct Builder {
	nodes: Vec<(u64, Node)>,
	next_value_hash: u64,
}

impl Builder {
	fn push(&mut self, identifier: &str, inputs: Vec<Input>) -> Input {
		self.push_implementation(Implementation::Proto(identifier.to_string()), inputs)
	}

	fn push_implementation(&mut self, implementation: Implementation, inputs: Vec<Input>) -> Input {
		let id = self.nodes.len() as u64 + 1;
		self.nodes.push((
			id,
			Node {
				inputs,
				call_argument: context(),
				implementation,
				visible: true,
				skip_deduplication: false,
				compile_metadata: None,
			},
		));
		Input::Node(NodeReference { node: id, output: 0 })
	}

	/// A distinct constant, so deduplication by stable ID does not collapse the workload.
	fn value(&mut self) -> Input {
		self.next_value_hash += 1;
		Input::Value(Value {
			ty: number(),
			hash: self.next_value_hash.wrapping_mul(0x9E37_79B9_7F4A_7C15),
			size: 8,
			exposed: false,
			producer: None,
		})
	}

	/// A leaf reading the animation time, which gives the nullification pass context features to track.
	fn time(&mut self) -> Input {
		self.push(ANIMATION_TIME, Vec::new())
	}

	fn add(&mut self, a: Input, b: Input) -> Input {
		self.push(ADD, vec![a, b])
	}

	/// Sums the inputs with a balanced tree of adds.
	fn reduce(&mut self, mut layer: Vec<Input>) -> Input {
		while layer.len() > 1 {
			let mut next = Vec::with_capacity(layer.len().div_ceil(2));
			let mut pairs = layer.into_iter();
			while let Some(a) = pairs.next() {
				next.push(match pairs.next() {
					Some(b) => self.add(a, b),
					None => a,
				});
			}
			layer = next;
		}
		layer.pop().expect("reduce needs at least one input")
	}

	fn finish(self, export: Input) -> Network {
		Network {
			exports: vec![export],
			nodes: self.nodes,
			scope_injections: Vec::new(),
			definitions: Vec::new(),
			generated: false,
		}
	}

	/// Finishes the document network and places it in the editor's scope wrapper.
	fn finish_document(self, export: Input) -> Network {
		wrap_in_scope(self.finish(export), false)
	}
}

/// A linear chain of `length` adds, alternating time and constant operands.
pub fn chain(length: usize) -> Network {
	let mut builder = Builder::default();
	let mut current = builder.time();
	for i in 0..length {
		let operand = if i % 2 == 0 { builder.value() } else { builder.time() };
		current = builder.add(current, operand);
	}
	builder.finish_document(current)
}

/// `depth` layers of `width` adds where each node sums two neighbors of the layer below, so paths reconverge.
pub fn lattice(width: usize, depth: usize) -> Network {
	let mut builder = Builder::default();
	let mut layer: Vec<Input> = (0..width).map(|i| if i % 2 == 0 { builder.time() } else { builder.value() }).collect();
	for _ in 0..depth {
		layer = (0..width).map(|i| builder.add(layer[i].clone(), layer[(i + 1) % width].clone())).collect();
	}
	let export = builder.reduce(layer);
	builder.finish_document(export)
}

/// One time leaf feeding `width` adds, summed by a balanced tree.
pub fn fan_out(width: usize) -> Network {
	let mut builder = Builder::default();
	let source = builder.time();
	let consumers = (0..width)
		.map(|_| {
			let operand = builder.value();
			builder.add(source.clone(), operand)
		})
		.collect();
	let export = builder.reduce(consumers);
	builder.finish_document(export)
}

/// A chain of `length` adds over a network import, used as the body of nested or instanced subnetworks.
fn import_chain(length: usize) -> Network {
	let mut builder = Builder::default();
	let mut current = Input::Import(Import { ty: number(), index: 0 });
	for i in 0..length {
		let operand = if i % 2 == 0 { builder.value() } else { builder.time() };
		current = builder.add(current, operand);
	}
	builder.finish(current)
}

/// Subnetworks nested `depth` levels deep, each adding to the result of the level inside it.
pub fn nesting(depth: usize, chain_length: usize) -> Network {
	let mut inner = import_chain(chain_length);
	for _ in 0..depth {
		let mut builder = Builder::default();
		let nested = builder.push_implementation(Implementation::Network(inner), vec![Input::Import(Import { ty: number(), index: 0 })]);
		let operand = builder.time();
		let export = builder.add(nested, operand);
		inner = builder.finish(export);
	}

	let mut builder = Builder::default();
	let source = builder.time();
	let export = builder.push_implementation(Implementation::Network(inner), vec![source]);
	builder.finish_document(export)
}

fn node_inputs(node: &Node) -> impl Iterator<Item = u64> + '_ {
	node.inputs.iter().filter_map(|input| match input {
		Input::Node(reference) => Some(reference.node),
		_ => None,
	})
}

/// The layer node and the content chain feeding its second input, as copied from a document the editor drew.
fn layer_group(document: &Network, layer: u64) -> Vec<u64> {
	let nodes: std::collections::HashMap<u64, &Node> = document.nodes.iter().map(|(id, node)| (*id, node)).collect();
	let Input::Node(content) = &nodes[&layer].inputs[1] else {
		panic!("A layer's second input should be its content")
	};

	let mut group = vec![layer];
	let mut stack = vec![content.node];
	while let Some(id) = stack.pop() {
		if group.contains(&id) {
			continue;
		}
		group.push(id);
		stack.extend(node_inputs(nodes[&id]));
	}
	group
}

/// `count` rectangle layers stacked on each other, replicating the top layer of the editor-drawn `rectangle-layers` template.
/// Every layer keeps the real node chain the Rectangle tool creates and the document keeps the editor's render pipeline.
pub fn rectangle_layers(count: usize) -> Network {
	let mut scoped = load_network("rectangle-layers");
	let (_, document_node) = scoped
		.nodes
		.iter_mut()
		.find(|(id, _)| *id == DOCUMENT_NODE)
		.expect("the scope wrapper should contain the document node");
	let Implementation::Network(document) = &mut document_node.implementation else {
		panic!("The document node should be a network")
	};

	let [Input::Node(top)] = document.exports.as_slice() else {
		panic!("The template should export its top layer")
	};
	let top_layer = top.node;
	let template: std::collections::HashMap<u64, Node> = document.nodes.iter().cloned().collect();

	// Walk down the layer stack to the bottom layer, whose stack input is a value
	let mut bottom_layer = top_layer;
	while let Input::Node(below) = &template[&bottom_layer].inputs[0] {
		bottom_layer = below.node;
	}

	let mut nodes: Vec<(u64, Node)> = layer_group(document, bottom_layer).into_iter().map(|id| (id, template[&id].clone())).collect();
	let top_group = layer_group(document, top_layer);

	// Values that differ between the top layer and the one below it (like a rectangle's position) are made distinct per copy
	let Input::Node(below_top) = &template[&top_layer].inputs[0] else {
		panic!("The template should stack at least two layers")
	};
	let below_group = layer_group(document, below_top.node);
	let varying_value = |id: u64, below_id: u64, input_index: usize| match (&template[&id].inputs[input_index], &template[&below_id].inputs[input_index]) {
		(Input::Value(top), Input::Value(below)) => top.hash != below.hash,
		_ => false,
	};

	let mut previous_layer = bottom_layer;
	for copy in 1..count as u64 {
		let remap = |id: u64| id.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(copy);
		for (&id, &below_id) in top_group.iter().zip(&below_group) {
			let mut node = template[&id].clone();
			for (input_index, input) in node.inputs.iter_mut().enumerate() {
				match input {
					Input::Node(reference) => reference.node = remap(reference.node),
					Input::Value(value) if varying_value(id, below_id, input_index) => value.hash = value.hash.wrapping_add(copy),
					_ => {}
				}
			}
			if id == top_layer {
				node.inputs[0] = Input::Node(NodeReference { node: previous_layer, output: 0 });
			}
			nodes.push((remap(id), node));
		}
		previous_layer = remap(top_layer);
	}

	document.nodes = nodes;
	document.exports = vec![Input::Node(NodeReference { node: previous_layer, output: 0 })];
	scoped
}

/// `count` instances of one subnetwork definition, each fed by its own time leaf and summed by a balanced tree.
/// The instances name one shared definitions table entry, as nodes inserted from the catalog do.
pub fn instances(count: usize, chain_length: usize) -> Network {
	const SHARED_DEFINITION: u64 = 1;

	let mut builder = Builder::default();
	let outputs = (0..count)
		.map(|_| {
			let source = builder.time();
			builder.push_implementation(Implementation::Definition(SHARED_DEFINITION), vec![source])
		})
		.collect();
	let export = builder.reduce(outputs);

	let mut document = builder.finish(export);
	document.definitions = vec![(SHARED_DEFINITION, import_chain(chain_length))];
	wrap_in_scope(document, false)
}
