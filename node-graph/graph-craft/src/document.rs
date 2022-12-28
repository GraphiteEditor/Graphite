use crate::generic;
use crate::proto::{ConstructionArgs, NodeIdentifier, ProtoNetwork, ProtoNode, ProtoNodeInput, Type};
use std::collections::HashMap;
use std::sync::Mutex;

pub mod value;

use dyn_any::{DynAny, StaticType};
use glam::IVec2;
use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaCha20Rng,
};

pub type NodeId = u64;
static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);

pub fn generate_uuid() -> u64 {
	let mut lock = RNG.lock().expect("uuid mutex poisoned");
	if lock.is_none() {
		*lock = Some(ChaCha20Rng::seed_from_u64(0));
	}
	lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
}

fn merge_ids(a: u64, b: u64) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	hasher.finish()
}

#[derive(Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentNodeMetadata {
	pub position: IVec2,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentNode {
	pub name: String,
	pub inputs: Vec<NodeInput>,
	pub implementation: DocumentNodeImplementation,
	pub metadata: DocumentNodeMetadata,
}

impl DocumentNode {
	pub fn populate_first_network_input(&mut self, node: NodeId, offset: usize) {
		let input = self
			.inputs
			.iter()
			.enumerate()
			.filter(|(_, input)| matches!(input, NodeInput::Network))
			.nth(offset)
			.expect("no network input");

		let index = input.0;
		self.inputs[index] = NodeInput::Node(node);
	}

	fn resolve_proto_node(mut self) -> ProtoNode {
		assert_ne!(self.inputs.len(), 0, "Resolving document node {:#?} with no inputs", self);
		let first = self.inputs.remove(0);
		if let DocumentNodeImplementation::Unresolved(fqn) = self.implementation {
			let (input, mut args) = match first {
				NodeInput::Value { tagged_value, .. } => {
					assert_eq!(self.inputs.len(), 0);
					(ProtoNodeInput::None, ConstructionArgs::Value(tagged_value.to_value()))
				}
				NodeInput::Node(id) => (ProtoNodeInput::Node(id), ConstructionArgs::Nodes(vec![])),
				NodeInput::Network => (ProtoNodeInput::Network, ConstructionArgs::Nodes(vec![])),
			};
			assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Network)), "recieved non resolved parameter");
			assert!(
				!self.inputs.iter().any(|input| matches!(input, NodeInput::Value { .. })),
				"recieved value as parameter. inupts: {:#?}, construction_args: {:#?}",
				&self.inputs,
				&args
			);

			if let ConstructionArgs::Nodes(nodes) = &mut args {
				nodes.extend(self.inputs.iter().map(|input| match input {
					NodeInput::Node(id) => *id,
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
			let &mut NodeInput::Node(id) = input else {
				continue;
			};
			if let Some(&new_id) = new_ids.get(&id) {
				*input = NodeInput::Node(new_id);
			} else if let Some(new_input) = default_input(self.name.clone(), index) {
				*input = new_input;
			} else {
				warn!("Node does not exist in library with that many inputs");
			}
		}
		self
	}
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeInput {
	Node(NodeId),
	Value { tagged_value: value::TaggedValue, exposed: bool },
	Network,
}

impl NodeInput {
	pub const fn value(tagged_value: value::TaggedValue, exposed: bool) -> Self {
		Self::Value { tagged_value, exposed }
	}
	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let NodeInput::Node(id) = self {
			*self = NodeInput::Node(f(*id))
		}
	}
	pub fn is_exposed(&self) -> bool {
		match self {
			NodeInput::Node(_) => true,
			NodeInput::Value { exposed, .. } => *exposed,
			NodeInput::Network => false,
		}
	}
}

impl PartialEq for NodeInput {
	fn eq(&self, other: &Self) -> bool {
		match (&self, &other) {
			(Self::Node(n1), Self::Node(n2)) => n1 == n2,
			(Self::Value { tagged_value: v1, .. }, Self::Value { tagged_value: v2, .. }) => v1 == v2,
			_ => core::mem::discriminant(self) == core::mem::discriminant(other),
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeNetwork {
	pub inputs: Vec<NodeId>,
	pub output: NodeId,
	pub nodes: HashMap<NodeId, DocumentNode>,
	/// These nodes are replaced with identity nodes when flattening
	pub disabled: Vec<NodeId>,
	/// In the case where a new node is chosen as output - what was the origional
	pub previous_output: Option<NodeId>,
}

impl NodeNetwork {
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.inputs.iter_mut().for_each(|id| *id = f(*id));
		self.output = f(self.output);
		self.disabled.iter_mut().for_each(|id| *id = f(*id));
		self.previous_output = self.previous_output.map(f);
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
				if let NodeInput::Node(ref_id) = input {
					outwards_links.entry(*ref_id).or_default().push(*node_id)
				}
			}
		}
		outwards_links
	}

	pub fn flatten(&mut self, node: NodeId) {
		self.flatten_with_fns(node, merge_ids, generate_uuid)
	}

	/// Recursively dissolve non primitive document nodes and return a single flattened network of nodes.
	pub fn flatten_with_fns(&mut self, node: NodeId, map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy, gen_id: impl Fn() -> NodeId + Copy) {
		let (id, mut node) = self
			.nodes
			.remove_entry(&node)
			.unwrap_or_else(|| panic!("The node which was supposed to be flattened does not exist in the network, id {} network {:#?}", node, self));

		if self.disabled.contains(&id) {
			node.implementation = DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")]));
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
						NodeInput::Node(node) => {
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.populate_first_network_input(node, *offset);
						}
						NodeInput::Value { tagged_value, exposed } => {
							let name = format!("Value: {:?}", tagged_value.clone().to_value());
							let new_id = map_ids(id, gen_id());
							let value_node = DocumentNode {
								name: name.clone(),
								inputs: vec![NodeInput::Value { tagged_value, exposed }],
								implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::value::ValueNode", &[generic!("T")])),
								metadata: DocumentNodeMetadata::default(),
							};
							assert!(!self.nodes.contains_key(&new_id));
							self.nodes.insert(new_id, value_node);
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.populate_first_network_input(new_id, *offset);
						}
						NodeInput::Network => {
							*network_offsets.get_mut(network_input).unwrap() += 1;
							if let Some(index) = self.inputs.iter().position(|i| *i == id) {
								self.inputs[index] = *network_input;
							}
						}
					}
				}
				node.implementation = DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")]));
				node.inputs = vec![NodeInput::Node(inner_network.output)];
				for node_id in new_nodes {
					self.flatten_with_fns(node_id, map_ids, gen_id);
				}
			}
			DocumentNodeImplementation::Unresolved(_) => (),
		}
		assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");
		self.nodes.insert(id, node);
	}

	pub fn into_proto_network(self) -> ProtoNetwork {
		let mut nodes: Vec<_> = self.nodes.into_iter().map(|(id, node)| (id, node.resolve_proto_node())).collect();
		nodes.sort_unstable_by_key(|(i, _)| *i);
		ProtoNetwork {
			inputs: self.inputs,
			output: self.output,
			nodes,
		}
	}

	/// Get the original output node of this network, ignoring any preview node
	pub fn original_output(&self) -> NodeId {
		self.previous_output.unwrap_or(self.output)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, NodeIdentifier, ProtoNetwork, ProtoNode, ProtoNodeInput};
	use value::IntoValue;

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
			output: 1,
			nodes: [
				(
					0,
					DocumentNode {
						name: "Cons".into(),
						inputs: vec![NodeInput::Network, NodeInput::Network],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")])),
					},
				),
				(
					1,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::Node(0)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode", &[generic!("T"), generic!("U")])),
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
			output: 2,
			nodes: [
				(
					1,
					DocumentNode {
						name: "Cons".into(),
						inputs: vec![NodeInput::Network, NodeInput::Network],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")])),
					},
				),
				(
					2,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::Node(1)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode", &[generic!("T"), generic!("U")])),
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
			output: 1,
			nodes: [(
				1,
				DocumentNode {
					name: "Inc".into(),
					inputs: vec![
						NodeInput::Network,
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
			inputs: vec![NodeInput::Network, NodeInput::Node(0)],
			metadata: DocumentNodeMetadata::default(),
			implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")])),
		};

		let proto_node = document_node.resolve_proto_node();
		let reference = ProtoNode {
			identifier: NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")]),
			input: ProtoNodeInput::Network,
			construction_args: ConstructionArgs::Nodes(vec![0]),
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
						identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")]),
						input: ProtoNodeInput::Node(11),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					10,
					ProtoNode {
						identifier: NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")]),
						input: ProtoNodeInput::Network,
						construction_args: ConstructionArgs::Nodes(vec![14]),
					},
				),
				(
					11,
					ProtoNode {
						identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[generic!("T"), generic!("U")]),
						input: ProtoNodeInput::Node(10),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(14, ProtoNode::value(ConstructionArgs::Value(2_u32.into_any()))),
			]
			.into_iter()
			.collect(),
		};
		let network = flat_network();
		let resolved_network = network.into_proto_network();

		println!("{:#?}", resolved_network);
		println!("{:#?}", construction_network);
		assert_eq!(resolved_network, construction_network);
	}

	fn flat_network() -> NodeNetwork {
		NodeNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					1,
					DocumentNode {
						name: "Inc".into(),
						inputs: vec![NodeInput::Node(11)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")])),
					},
				),
				(
					10,
					DocumentNode {
						name: "Cons".into(),
						inputs: vec![NodeInput::Network, NodeInput::Node(14)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("T"), generic!("U")])),
					},
				),
				(
					14,
					DocumentNode {
						name: "Value: 2".into(),
						inputs: vec![NodeInput::Value {
							tagged_value: value::TaggedValue::U32(2),
							exposed: false,
						}],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::value::ValueNode", &[generic!("T")])),
					},
				),
				(
					11,
					DocumentNode {
						name: "Add".into(),
						inputs: vec![NodeInput::Node(10)],
						metadata: DocumentNodeMetadata::default(),
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::AddNode", &[generic!("T"), generic!("U")])),
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}
}
