use crate::document::value::TaggedValue;
use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};
use graphene_core::{NodeIdentifier, Type};

use dyn_any::{DynAny, StaticType};
use glam::IVec2;
pub use graphene_core::uuid::generate_uuid;
use graphene_core::TypeDescriptor;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

pub mod value;

pub type NodeId = u64;

fn merge_ids(a: u64, b: u64) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	hasher.finish()
}

#[derive(Clone, Debug, PartialEq, Default, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentNodeMetadata {
	pub position: IVec2,
}

impl DocumentNodeMetadata {
	pub fn position(position: impl Into<IVec2>) -> Self {
		Self { position: position.into() }
	}
}

#[derive(Clone, Debug, PartialEq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentNode {
	pub name: String,
	pub inputs: Vec<NodeInput>,
	pub implementation: DocumentNodeImplementation,
	pub metadata: DocumentNodeMetadata,
}

impl DocumentNode {
	pub fn populate_first_network_input(&mut self, node_id: NodeId, output_index: usize, offset: usize, lambda: bool) {
		let input = self
			.inputs
			.iter()
			.enumerate()
			.filter(|(_, input)| matches!(input, NodeInput::Network(_)))
			.nth(offset)
			.unwrap_or_else(|| panic!("no network input found for {self:#?} and offset: {offset}"));

		let index = input.0;
		self.inputs[index] = NodeInput::Node { node_id, output_index, lambda };
	}

	fn resolve_proto_node(mut self) -> ProtoNode {
		assert_ne!(self.inputs.len(), 0, "Resolving document node {:#?} with no inputs", self);
		let first = self.inputs.remove(0);
		if let DocumentNodeImplementation::Unresolved(fqn) = self.implementation {
			let (input, mut args) = match first {
				NodeInput::Value { tagged_value, .. } => {
					assert_eq!(self.inputs.len(), 0, "{}, {:?}", &self.name, &self.inputs);
					(ProtoNodeInput::None, ConstructionArgs::Value(tagged_value))
				}
				NodeInput::Node { node_id, output_index, lambda } => {
					assert_eq!(output_index, 0, "Outputs should be flattened before converting to protonode.");
					(ProtoNodeInput::Node(node_id, lambda), ConstructionArgs::Nodes(vec![]))
				}
				NodeInput::Network(ty) => (ProtoNodeInput::Network(ty), ConstructionArgs::Nodes(vec![])),
			};
			assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Network(_))), "recieved non resolved parameter");
			assert!(
				!self.inputs.iter().any(|input| matches!(input, NodeInput::Value { .. })),
				"recieved value as parameter. inupts: {:#?}, construction_args: {:#?}",
				&self.inputs,
				&args
			);

			if let ConstructionArgs::Nodes(nodes) = &mut args {
				nodes.extend(self.inputs.iter().map(|input| match input {
					NodeInput::Node { node_id, lambda, .. } => (*node_id, *lambda),
					_ => unreachable!(),
				}));
			}
			ProtoNode {
				identifier: fqn,
				input,
				construction_args: args,
			}
		} else {
			unreachable!("tried to resolve not flattened node on resolved node");
		}
	}

	/// Converts all node id inputs to a new id based on a HashMap.
	///
	/// If the node is not in the hashmap then a default input is found based on the node name and input index.
	pub fn map_ids<P>(mut self, default_input: P, new_ids: &HashMap<NodeId, NodeId>) -> Self
	where
		P: Fn(String, usize) -> Option<NodeInput>,
	{
		for (index, input) in self.inputs.iter_mut().enumerate() {
			let &mut NodeInput::Node{node_id: id, output_index, lambda} = input else {
				continue;
			};
			if let Some(&new_id) = new_ids.get(&id) {
				*input = NodeInput::Node {
					node_id: new_id,
					output_index,
					lambda,
				};
			} else if let Some(new_input) = default_input(self.name.clone(), index) {
				*input = new_input;
			} else {
				warn!("Node does not exist in library with that many inputs");
			}
		}
		self
	}
}

#[derive(Debug, Clone, PartialEq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeInput {
	Node { node_id: NodeId, output_index: usize, lambda: bool },
	Value { tagged_value: value::TaggedValue, exposed: bool },
	Network(Type),
}

impl NodeInput {
	pub const fn node(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index, lambda: false }
	}
	pub const fn lambda(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index, lambda: true }
	}
	pub const fn value(tagged_value: value::TaggedValue, exposed: bool) -> Self {
		Self::Value { tagged_value, exposed }
	}
	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let &mut NodeInput::Node { node_id, output_index, lambda } = self {
			*self = NodeInput::Node {
				node_id: f(node_id),
				output_index,
				lambda,
			}
		}
	}
	pub fn is_exposed(&self) -> bool {
		match self {
			NodeInput::Node { .. } => true,
			NodeInput::Value { exposed, .. } => *exposed,
			NodeInput::Network(_) => false,
		}
	}
	pub fn ty(&self) -> Type {
		match self {
			NodeInput::Node { .. } => unreachable!("ty() called on NodeInput::Node"),
			NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
			NodeInput::Network(ty) => ty.clone(),
		}
	}
}

#[derive(Clone, Debug, PartialEq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DocumentNodeImplementation {
	Network(NodeNetwork),
	Unresolved(NodeIdentifier),
}

impl DocumentNodeImplementation {
	pub fn get_network(&self) -> Option<&NodeNetwork> {
		if let DocumentNodeImplementation::Network(n) = self {
			Some(n)
		} else {
			None
		}
	}

	pub fn get_network_mut(&mut self) -> Option<&mut NodeNetwork> {
		if let DocumentNodeImplementation::Network(n) = self {
			Some(n)
		} else {
			None
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeOutput {
	pub node_id: NodeId,
	pub node_output_index: usize,
}
impl NodeOutput {
	pub fn new(node_id: NodeId, node_output_index: usize) -> Self {
		Self { node_id, node_output_index }
	}
}

#[derive(Clone, Debug, Default, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeNetwork {
	pub inputs: Vec<NodeId>,
	pub outputs: Vec<NodeOutput>,
	pub nodes: HashMap<NodeId, DocumentNode>,
	/// These nodes are replaced with identity nodes when flattening
	pub disabled: Vec<NodeId>,
	/// In the case where a new node is chosen as output - what was the original
	pub previous_outputs: Option<Vec<NodeOutput>>,
}

impl NodeNetwork {
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.inputs.iter_mut().for_each(|id| *id = f(*id));
		self.outputs.iter_mut().for_each(|output| output.node_id = f(output.node_id));
		self.disabled.iter_mut().for_each(|id| *id = f(*id));
		self.previous_outputs
			.iter_mut()
			.for_each(|nodes| nodes.iter_mut().for_each(|output| output.node_id = f(output.node_id)));
		let mut empty = HashMap::new();
		std::mem::swap(&mut self.nodes, &mut empty);
		self.nodes = empty
			.into_iter()
			.map(|(id, mut node)| {
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				(f(id), node)
			})
			.collect();
	}

	/// Collect a hashmap of nodes with a list of the nodes that use it as input
	pub fn collect_outwards_links(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut outwards_links: HashMap<u64, Vec<u64>> = HashMap::new();
		for (node_id, node) in &self.nodes {
			for input in &node.inputs {
				if let NodeInput::Node { node_id: ref_id, .. } = input {
					outwards_links.entry(*ref_id).or_default().push(*node_id)
				}
			}
		}
		outwards_links
	}

	/// When a node has multiple outputs, we actually just duplicate the node and evaluate each output separately
	pub fn duplicate_outputs(&mut self, mut gen_id: &mut impl FnMut() -> NodeId) {
		let mut duplicating_nodes = HashMap::new();
		// Find the nodes where the inputs require duplicating
		for node in &mut self.nodes.values_mut() {
			// Recursivly duplicate children
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				network.duplicate_outputs(gen_id);
			}

			for input in &mut node.inputs {
				let &mut NodeInput::Node { node_id, output_index, .. } = input else {
					continue;
				};
				// Use the initial node when getting the first output
				if output_index == 0 {
					continue;
				}
				// Get the existing duplicated node id (or create a new one)
				let duplicated_node_id = *duplicating_nodes.entry((node_id, output_index)).or_insert_with(&mut gen_id);
				// Use the first output from the duplicated node
				*input = NodeInput::node(duplicated_node_id, 0);
			}
		}
		// Find the network outputs that require duplicating
		for network_output in &mut self.outputs {
			// Use the initial node when getting the first output
			if network_output.node_output_index == 0 {
				continue;
			}
			// Get the existing duplicated node id (or create a new one)
			let duplicated_node_id = *duplicating_nodes.entry((network_output.node_id, network_output.node_output_index)).or_insert_with(&mut gen_id);
			// Use the first output from the duplicated node
			*network_output = NodeOutput::new(duplicated_node_id, 0);
		}
		// Duplicate the nodes
		for ((original_node_id, output_index), new_node_id) in duplicating_nodes {
			let Some(original_node) = self.nodes.get(&original_node_id) else {
				continue;
			};
			let mut new_node = original_node.clone();
			// Update the required outputs from a nested network to be just the relevant output
			if let DocumentNodeImplementation::Network(network) = &mut new_node.implementation {
				if network.outputs.is_empty() {
					continue;
				}
				network.outputs = vec![network.outputs[output_index]];
			}
			self.nodes.insert(new_node_id, new_node);
		}

		// Ensure all nodes only have one output
		for node in self.nodes.values_mut() {
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				if network.outputs.is_empty() {
					continue;
				}
				network.outputs = vec![network.outputs[0]];
			}
		}
	}

	/// Removes unused nodes from the graph. Returns a list of bools which represent if each of the inputs have been retained
	pub fn remove_dead_nodes(&mut self) -> Vec<bool> {
		// Take all the nodes out of the nodes list
		let mut old_nodes = std::mem::take(&mut self.nodes);
		let mut stack = self.outputs.iter().map(|output| output.node_id).collect::<Vec<_>>();
		while let Some(node_id) = stack.pop() {
			let Some((node_id, mut document_node)) = old_nodes.remove_entry(&node_id) else {
				continue;
			};
			// Remove dead nodes from child networks
			if let DocumentNodeImplementation::Network(network) = &mut document_node.implementation {
				// Remove inputs to the parent node if they have been removed from the child
				let mut retain_inputs = network.remove_dead_nodes().into_iter();
				document_node.inputs.retain(|_| retain_inputs.next().unwrap_or(true))
			}
			// Visit all nodes that this node references
			stack.extend(
				document_node
					.inputs
					.iter()
					.filter_map(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None }),
			);
			// Add the node back to the list of nodes
			self.nodes.insert(node_id, document_node);
		}

		// Check if inputs are used and store for return value
		let are_inputs_used = self.inputs.iter().map(|input| self.nodes.contains_key(input)).collect();
		// Remove unused inputs from graph
		self.inputs.retain(|input| self.nodes.contains_key(input));

		are_inputs_used
	}

	pub fn flatten(&mut self, node: NodeId) {
		self.flatten_with_fns(node, merge_ids, generate_uuid)
	}

	/// Recursively dissolve non-primitive document nodes and return a single flattened network of nodes.
	pub fn flatten_with_fns(&mut self, node: NodeId, map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy, gen_id: impl Fn() -> NodeId + Copy) {
		let (id, mut node) = self
			.nodes
			.remove_entry(&node)
			.unwrap_or_else(|| panic!("The node which was supposed to be flattened does not exist in the network, id {} network {:#?}", node, self));

		if self.disabled.contains(&id) {
			node.implementation = DocumentNodeImplementation::Unresolved("graphene_core::ops::IdNode".into());
			node.inputs.drain(1..);
			self.nodes.insert(id, node);
			return;
		}

		match node.implementation {
			DocumentNodeImplementation::Network(mut inner_network) => {
				// Connect all network inputs to either the parent network nodes, or newly created value nodes.
				inner_network.map_ids(|inner_id| map_ids(id, inner_id));
				let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();
				// Copy nodes from the inner network into the parent network
				self.nodes.extend(inner_network.nodes);
				self.disabled.extend(inner_network.disabled);

				let mut network_offsets = HashMap::new();
				for (document_input, network_input) in node.inputs.into_iter().zip(inner_network.inputs.iter()) {
					let offset = network_offsets.entry(network_input).or_insert(0);
					match document_input {
						NodeInput::Node { node_id, output_index, lambda } => {
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.populate_first_network_input(node_id, output_index, *offset, lambda);
						}
						NodeInput::Value { tagged_value, exposed } => {
							// Skip formatting very large values for seconds in performance speedup
							let name = if matches!(
								tagged_value,
								TaggedValue::Image(_) | TaggedValue::RcImage(_) | TaggedValue::Color(_) | TaggedValue::Subpath(_) | TaggedValue::RcSubpath(_)
							) {
								"Value".to_string()
							} else {
								format!("Value: {:?}", tagged_value)
							};
							let new_id = map_ids(id, gen_id());
							let value_node = DocumentNode {
								name,
								inputs: vec![NodeInput::Value { tagged_value, exposed }],
								implementation: DocumentNodeImplementation::Unresolved("graphene_core::value::ValueNode".into()),
								metadata: DocumentNodeMetadata::default(),
							};
							assert!(!self.nodes.contains_key(&new_id));
							self.nodes.insert(new_id, value_node);
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.populate_first_network_input(new_id, 0, *offset, false);
						}
						NodeInput::Network(_) => {
							*network_offsets.get_mut(network_input).unwrap() += 1;
							if let Some(index) = self.inputs.iter().position(|i| *i == id) {
								self.inputs[index] = *network_input;
							}
						}
					}
				}
				node.implementation = DocumentNodeImplementation::Unresolved("graphene_core::ops::IdNode".into());
				node.inputs = inner_network
					.outputs
					.iter()
					.map(|&NodeOutput { node_id, node_output_index }| NodeInput::Node {
						node_id,
						output_index: node_output_index,
						lambda: false,
					})
					.collect();

				for node_id in new_nodes {
					self.flatten_with_fns(node_id, map_ids, gen_id);
				}
			}
			DocumentNodeImplementation::Unresolved(_) => (),
		}
		assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");
		self.nodes.insert(id, node);
	}

	pub fn into_proto_networks(self) -> impl Iterator<Item = ProtoNetwork> {
		let mut nodes: Vec<_> = self.nodes.into_iter().map(|(id, node)| (id, node.resolve_proto_node())).collect();
		nodes.sort_unstable_by_key(|(i, _)| *i);

		// Create a network to evaluate each output
		self.outputs.into_iter().map(move |output| ProtoNetwork {
			inputs: self.inputs.clone(),
			output: output.node_id,
			nodes: nodes.clone(),
		})
	}

	/// Get the original output nodes of this network, ignoring any preview node
	pub fn original_outputs(&self) -> &Vec<NodeOutput> {
		self.previous_outputs.as_ref().unwrap_or(&self.outputs)
	}

	/// A graph with just an input and output node
	pub fn new_network(output_offset: i32, output_node_id: NodeId) -> Self {
		Self {
			inputs: vec![0],
			outputs: vec![NodeOutput::new(1, 0)],
			nodes: [
				(
					0,
					DocumentNode {
						name: "Input".into(),
						inputs: vec![NodeInput::Network(concrete!(u32))],
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::ops::IdNode".into()),
						metadata: DocumentNodeMetadata { position: (8, 4).into() },
					},
				),
				(
					1,
					DocumentNode {
						name: "Output".into(),
						inputs: vec![NodeInput::node(output_node_id, 0)],
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::ops::IdNode".into()),
						metadata: DocumentNodeMetadata { position: (output_offset, 4).into() },
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	/// Get the nested network given by the path of node ids
	pub fn nested_network(&self, nested_path: &[NodeId]) -> Option<&Self> {
		let mut network = Some(self);

		for segment in nested_path {
			network = network.and_then(|network| network.nodes.get(segment)).and_then(|node| node.implementation.get_network());
		}
		network
	}

	/// Get the mutable nested network given by the path of node ids
	pub fn nested_network_mut(&mut self, nested_path: &[NodeId]) -> Option<&mut Self> {
		let mut network = Some(self);

		for segment in nested_path {
			network = network.and_then(|network| network.nodes.get_mut(segment)).and_then(|node| node.implementation.get_network_mut());
		}
		network
	}

	/// Check if the specified node id is connected to the output
	pub fn connected_to_output(&self, target_node_id: NodeId) -> bool {
		// If the node is the output then return true
		if self.outputs.iter().any(|&NodeOutput { node_id, .. }| node_id == target_node_id) {
			return true;
		}
		// Get the outputs
		let Some(mut stack) = self.outputs.iter().map(|&output| self.nodes.get(&output.node_id)).collect::<Option<Vec<_>>>() else {
			return false;
		};
		let mut already_visited = HashSet::new();
		already_visited.extend(self.outputs.iter().map(|output| output.node_id));

		while let Some(node) = stack.pop() {
			for input in &node.inputs {
				if let &NodeInput::Node { node_id: ref_id, .. } = input {
					// Skip if already viewed
					if already_visited.contains(&ref_id) {
						continue;
					}
					// If the target node is used as input then return true
					if ref_id == target_node_id {
						return true;
					}
					// Add the referenced node to the stack
					let Some(ref_node) = self.nodes.get(&ref_id) else {
						continue;
					};
					already_visited.insert(ref_id);
					stack.push(ref_node);
				}
			}
		}

		false
	}

	/// Is the node being used directly as an output?
	pub fn outputs_contain(&self, node_id: NodeId) -> bool {
		self.outputs.iter().any(|output| output.node_id == node_id)
	}

	/// Is the node being used directly as an original output?
	pub fn original_outputs_contain(&self, node_id: NodeId) -> bool {
		self.original_outputs().iter().any(|output| output.node_id == node_id)
	}

	/// Is the node being used directly as a previous output?
	pub fn previous_outputs_contain(&self, node_id: NodeId) -> Option<bool> {
		self.previous_outputs.as_ref().map(|outputs| outputs.iter().any(|output| output.node_id == node_id))
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};
	use graphene_core::NodeIdentifier;

	fn gen_node_id() -> NodeId {
		static mut NODE_ID: NodeId = 3;
		unsafe {
			NODE_ID += 1;
			NODE_ID
		}
	}

	fn add_network() -> NodeNetwork {
		NodeNetwork {
			inputs: vec![0, 0],
			outputs: vec![NodeOutput::new(1, 0)],
			nodes: [
				(
					0,
					DocumentNode {
						name: "Cons".into(),
						inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(u32))],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::structural::ConsNode".into()),
					},
				),
				(
					1,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::node(0, 0)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::ops::AddNode".into()),
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
		network.map_ids(|id| id + 1);
		let maped_add = NodeNetwork {
			inputs: vec![1, 1],
			outputs: vec![NodeOutput::new(2, 0)],
			nodes: [
				(
					1,
					DocumentNode {
						name: "Cons".into(),
						inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(u32))],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::structural::ConsNode".into()),
					},
				),
				(
					2,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::node(1, 0)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::ops::AddNode".into()),
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};
		assert_eq!(network, maped_add);
	}

	#[test]
	fn flatten_add() {
		let mut network = NodeNetwork {
			inputs: vec![1],
			outputs: vec![NodeOutput::new(1, 0)],
			nodes: [(
				1,
				DocumentNode {
					name: "Inc".into(),
					inputs: vec![
						NodeInput::Network(concrete!(u32)),
						NodeInput::Value {
							tagged_value: value::TaggedValue::U32(2),
							exposed: false,
						},
					],
					implementation: DocumentNodeImplementation::Network(add_network()),
					metadata: DocumentNodeMetadata::default(),
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};
		network.flatten_with_fns(1, |self_id, inner_id| self_id * 10 + inner_id, gen_node_id);
		let flat_network = flat_network();

		println!("{:#?}", network);
		println!("{:#?}", flat_network);
		assert_eq!(flat_network, network);
	}

	#[test]
	fn resolve_proto_node_add() {
		let document_node = DocumentNode {
			name: "Cons".into(),
			inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::node(0, 0)],
			metadata: DocumentNodeMetadata::default(),
			implementation: DocumentNodeImplementation::Unresolved("graphene_core::structural::ConsNode".into()),
		};

		let proto_node = document_node.resolve_proto_node();
		let reference = ProtoNode {
			identifier: "graphene_core::structural::ConsNode".into(),
			input: ProtoNodeInput::Network(concrete!(u32)),
			construction_args: ConstructionArgs::Nodes(vec![(0, false)]),
		};
		assert_eq!(proto_node, reference);
	}

	#[test]
	fn resolve_flatten_add_as_proto_network() {
		let construction_network = ProtoNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					1,
					ProtoNode {
						identifier: "graphene_core::ops::IdNode".into(),
						input: ProtoNodeInput::Node(11, false),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					10,
					ProtoNode {
						identifier: "graphene_core::structural::ConsNode".into(),
						input: ProtoNodeInput::Network(concrete!(u32)),
						construction_args: ConstructionArgs::Nodes(vec![(14, false)]),
					},
				),
				(
					11,
					ProtoNode {
						identifier: "graphene_core::ops::AddNode".into(),
						input: ProtoNodeInput::Node(10, false),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(14, ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2)))),
			]
			.into_iter()
			.collect(),
		};
		let network = flat_network();
		let resolved_network = network.into_proto_networks().collect::<Vec<_>>();

		println!("{:#?}", resolved_network[0]);
		println!("{:#?}", construction_network);
		assert_eq!(resolved_network[0], construction_network);
	}

	fn flat_network() -> NodeNetwork {
		NodeNetwork {
			inputs: vec![10],
			outputs: vec![NodeOutput::new(1, 0)],
			nodes: [
				(
					1,
					DocumentNode {
						name: "Inc".into(),
						inputs: vec![NodeInput::node(11, 0)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::ops::IdNode".into()),
					},
				),
				(
					10,
					DocumentNode {
						name: "Cons".into(),
						inputs: vec![NodeInput::Network(concrete!(u32)), NodeInput::node(14, 0)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::structural::ConsNode".into()),
					},
				),
				(
					14,
					DocumentNode {
						name: "Value: U32(2)".into(),
						inputs: vec![NodeInput::Value {
							tagged_value: value::TaggedValue::U32(2),
							exposed: false,
						}],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::value::ValueNode".into()),
					},
				),
				(
					11,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::node(10, 0)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved("graphene_core::ops::AddNode".into()),
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	fn two_node_identity() -> NodeNetwork {
		NodeNetwork {
			inputs: vec![1, 2],
			outputs: vec![NodeOutput::new(1, 0), NodeOutput::new(2, 0)],
			nodes: [
				(
					1,
					DocumentNode {
						name: "Identity 1".into(),
						inputs: vec![NodeInput::Network(concrete!(u32))],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
					},
				),
				(
					2,
					DocumentNode {
						name: "Identity 2".into(),
						inputs: vec![NodeInput::Network(concrete!(u32))],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	fn output_duplicate(network_outputs: Vec<NodeOutput>, result_node_input: NodeInput) -> NodeNetwork {
		let mut network = NodeNetwork {
			inputs: Vec::new(),
			outputs: network_outputs,
			nodes: [
				(
					10,
					DocumentNode {
						name: "Nested network".into(),
						inputs: vec![NodeInput::value(TaggedValue::F32(1.), false), NodeInput::value(TaggedValue::F32(2.), false)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Network(two_node_identity()),
					},
				),
				(
					11,
					DocumentNode {
						name: "Result".into(),
						inputs: vec![result_node_input],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};
		let mut new_ids = 101..;
		network.duplicate_outputs(&mut || new_ids.next().unwrap());
		network.remove_dead_nodes();
		network
	}

	#[test]
	fn simple_duplicate() {
		let result = output_duplicate(vec![NodeOutput::new(10, 1)], NodeInput::node(10, 0));
		assert_eq!(result.outputs.len(), 1, "The number of outputs should remain as 1");
		assert_eq!(result.outputs[0], NodeOutput::new(101, 0), "The outer network output should be from a duplicated inner network");
		assert_eq!(result.nodes.keys().copied().collect::<Vec<_>>(), vec![101], "Should just call nested network");
		let nested_network_node = result.nodes.get(&101).unwrap();
		assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
		assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(2.), false)], "Input should be 2");
		let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
		assert_eq!(inner_network.inputs, vec![2], "The input should be sent to the second node");
		assert_eq!(inner_network.outputs, vec![NodeOutput::new(2, 0)], "The output should be node id 2");
		assert_eq!(inner_network.nodes.get(&2).unwrap().name, "Identity 2", "The node should be identity 2");
	}

	#[test]
	fn out_of_order_duplicate() {
		let result = output_duplicate(vec![NodeOutput::new(10, 1), NodeOutput::new(10, 0)], NodeInput::node(10, 0));
		assert_eq!(result.outputs[0], NodeOutput::new(101, 0), "The first network output should be from a duplicated nested network");
		assert_eq!(result.outputs[1], NodeOutput::new(10, 0), "The second network output should be from the original nested network");
		assert!(
			result.nodes.contains_key(&10) && result.nodes.contains_key(&101) && result.nodes.len() == 2,
			"Network should contain two duplicated nodes"
		);
		for (node_id, input_value, inner_id) in [(10, 1., 1), (101, 2., 2)] {
			let nested_network_node = result.nodes.get(&node_id).unwrap();
			assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
			assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(input_value), false)], "Input should be stable");
			let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
			assert_eq!(inner_network.inputs, vec![inner_id], "The input should be sent to the second node");
			assert_eq!(inner_network.outputs, vec![NodeOutput::new(inner_id, 0)], "The output should be node id");
			assert_eq!(inner_network.nodes.get(&inner_id).unwrap().name, format!("Identity {inner_id}"), "The node should be identity");
		}
	}
	#[test]
	fn using_other_node_duplicate() {
		let result = output_duplicate(vec![NodeOutput::new(11, 0)], NodeInput::node(10, 1));
		assert_eq!(result.outputs, vec![NodeOutput::new(11, 0)], "The network output should be the result node");
		assert!(
			result.nodes.contains_key(&11) && result.nodes.contains_key(&101) && result.nodes.len() == 2,
			"Network should contain a duplicated node and a result node"
		);
		let result_node = result.nodes.get(&11).unwrap();
		assert_eq!(result_node.inputs, vec![NodeInput::node(101, 0)], "Result node should refer to duplicate node as input");
		let nested_network_node = result.nodes.get(&101).unwrap();
		assert_eq!(nested_network_node.name, "Nested network".to_string(), "Name should not change");
		assert_eq!(nested_network_node.inputs, vec![NodeInput::value(TaggedValue::F32(2.), false)], "Input should be 2");
		let inner_network = nested_network_node.implementation.get_network().expect("Implementation should be network");
		assert_eq!(inner_network.inputs, vec![2], "The input should be sent to the second node");
		assert_eq!(inner_network.outputs, vec![NodeOutput::new(2, 0)], "The output should be node id 2");
		assert_eq!(inner_network.nodes.get(&2).unwrap().name, "Identity 2", "The node should be identity 2");
	}
}
