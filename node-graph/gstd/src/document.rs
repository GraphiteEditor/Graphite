use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Mutex;

use dyn_any::{DynAny, StaticType};
use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaCha20Rng,
};

type NodeId = u64;
static RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);

pub fn generate_uuid() -> u64 {
	let mut lock = RNG.lock().expect("uuid mutex poisoned");
	if lock.is_none() {
		*lock = Some(ChaCha20Rng::seed_from_u64(0));
	}
	lock.as_mut().map(ChaCha20Rng::next_u64).unwrap()
}

fn gen_node_id() -> NodeId {
	static mut NODE_ID: NodeId = 3;
	unsafe {
		NODE_ID += 1;
		NODE_ID
	}
}

fn merge_ids(a: u64, b: u64) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug, PartialEq)]
pub struct DocumentNode {
	name: String,
	inputs: Vec<NodeInput>,
	implementation: DocumentNodeImplementation,
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

	fn resolve_proto_nodes(&mut self) {
		let first = self.inputs.remove(0);
		if let DocumentNodeImplementation::ProtoNode(proto) = &mut self.implementation {
			match first {
				NodeInput::Value(value) => {
					proto.input = ProtoNodeInput::None;
					proto.construction_args = ConstructionArgs::Value(value);
					assert_eq!(self.inputs.len(), 0);
					return;
				}
				NodeInput::Node(id) => proto.input = ProtoNodeInput::Node(id),
				NodeInput::Network => proto.input = ProtoNodeInput::Network,
			}
			assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Network)), "recived non resolved parameter");
			assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Value(_))), "recieved value as parameter");

			let nodes: Vec<_> = self
				.inputs
				.iter()
				.filter_map(|input| match input {
					NodeInput::Node(id) => Some(*id),
					_ => None,
				})
				.collect();
			match nodes {
				vec if vec.is_empty() => proto.construction_args = ConstructionArgs::None,
				vec => proto.construction_args = ConstructionArgs::Nodes(vec),
			}
			self.inputs = vec![];
		}
	}
}

#[derive(Debug)]
pub enum NodeInput {
	Node(NodeId),
	Value(Value),
	Network,
}

impl PartialEq for NodeInput {
	fn eq(&self, other: &Self) -> bool {
		match (&self, &other) {
			(Self::Node(n1), Self::Node(n2)) => n1 == n2,
			(Self::Value(v1), Self::Value(v2)) => v1 == v2,
			_ => core::mem::discriminant(self) == core::mem::discriminant(other),
		}
	}
}

#[derive(Debug, PartialEq)]
pub enum DocumentNodeImplementation {
	Network(NodeNetwork),
	ProtoNode(ProtoNode),
}

#[derive(Debug, Default, PartialEq)]
pub struct NodeNetwork {
	inputs: Vec<NodeId>,
	output: NodeId,
	nodes: HashMap<NodeId, DocumentNode>,
}
pub type Value = Box<dyn ValueTrait>;
pub trait ValueTrait: DynAny<'static> + std::fmt::Debug {}

pub trait IntoValue: Sized + ValueTrait + 'static {
	fn into_any(self) -> Value {
		Box::new(self)
	}
}
impl<T: 'static + StaticType + std::fmt::Debug + PartialEq> ValueTrait for T {}
impl<T: 'static + ValueTrait> IntoValue for T {}

#[repr(C)]
struct Vtable {
	destructor: unsafe fn(*mut ()),
	size: usize,
	align: usize,
}

#[repr(C)]
struct TraitObject {
	self_ptr: *mut u8,
	vtable: &'static Vtable,
}

impl PartialEq for Box<dyn ValueTrait> {
	fn eq(&self, other: &Self) -> bool {
		if self.type_id() != other.type_id() {
			return false;
		}
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let other_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(other.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) };
		let other_mem = unsafe { std::slice::from_raw_parts(other_trait_object.self_ptr, size) };
		self_mem == other_mem
	}
}

#[derive(Debug, Default)]
pub enum ConstructionArgs {
	None,
	#[default]
	Unresolved,
	Value(Value),
	Nodes(Vec<NodeId>),
}

impl PartialEq for ConstructionArgs {
	fn eq(&self, other: &Self) -> bool {
		match (&self, &other) {
			(Self::Nodes(n1), Self::Nodes(n2)) => n1 == n2,
			(Self::Value(v1), Self::Value(v2)) => v1 == v2,
			_ => core::mem::discriminant(self) == core::mem::discriminant(other),
		}
	}
}

#[derive(Debug, Default, PartialEq)]
pub struct ProtoNode {
	construction_args: ConstructionArgs,
	input: ProtoNodeInput,
	name: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum ProtoNodeInput {
	None,
	#[default]
	Network,
	Node(NodeId),
}

impl NodeInput {
	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let NodeInput::Node(id) = self {
			*self = NodeInput::Node(f(*id))
		}
	}
}

impl ProtoNode {
	pub fn id() -> Self {
		Self {
			name: "id".into(),
			..Default::default()
		}
	}
	pub fn unresolved(name: String) -> Self {
		Self { name, ..Default::default() }
	}
	pub fn value(name: String, value: ConstructionArgs) -> Self {
		Self {
			name,
			construction_args: value,
			..Default::default()
		}
	}
}

impl NodeNetwork {
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.inputs.iter_mut().for_each(|id| *id = f(*id));
		self.output = f(self.output);
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

	pub fn flatten(&mut self, node: NodeId) {
		self.flatten_with_fns(node, merge_ids, generate_uuid)
	}

	/// Recursively dissolve non primitive document nodes and return a single flattened network of nodes.
	pub fn flatten_with_fns(&mut self, node: NodeId, map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy, gen_id: impl Fn() -> NodeId + Copy) {
		let (id, mut node) = self.nodes.remove_entry(&node).expect("The node which was supposed to be flattened does not exist in the network");

		match node.implementation {
			DocumentNodeImplementation::Network(mut inner_network) => {
				// Connect all network inputs to either the parent network nodes, or newly created value nodes.
				inner_network.map_ids(|inner_id| map_ids(id, inner_id));
				let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();
				// Copy nodes from the inner network into the parent network
				self.nodes.extend(inner_network.nodes);

				let mut network_offsets = HashMap::new();
				for (document_input, network_input) in node.inputs.into_iter().zip(inner_network.inputs.iter()) {
					let offset = network_offsets.entry(network_input).or_insert(0);
					match document_input {
						NodeInput::Node(node) => {
							let network_input = self.nodes.get_mut(network_input).unwrap();
							network_input.populate_first_network_input(node, *offset);
						}
						NodeInput::Value(value) => {
							let name = format!("Value: {:?}", value);
							let new_id = map_ids(id, gen_id());
							let value_node = DocumentNode {
								name: name.clone(),
								inputs: vec![NodeInput::Value(value)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::unresolved("value".into())),
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
				node.implementation = DocumentNodeImplementation::ProtoNode(ProtoNode::id());
				node.inputs = vec![NodeInput::Node(inner_network.output)];
				for node_id in new_nodes {
					self.flatten_with_fns(node_id, map_ids, gen_id);
				}
			}
			DocumentNodeImplementation::ProtoNode(proto_node) => {
				node.implementation = DocumentNodeImplementation::ProtoNode(proto_node);
			}
		}
		assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");
		self.nodes.insert(id, node);
	}

	pub fn resolve_proto_nodes(&mut self) {
		for node in self.nodes.values_mut() {
			node.resolve_proto_nodes();
		}
	}
}

struct Map<I, O>(core::marker::PhantomData<(I, O)>);

impl<O> Display for Map<(), O> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
		write!(f, "Map")
	}
}

impl Display for Map<i32, String> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
		write!(f, "Map<String>")
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_any_src() {
		assert!(2_u32.into_any() == 2_u32.into_any());
		assert!(2_u32.into_any() != 3_u32.into_any());
		assert!(2_u32.into_any() != 3_i32.into_any());
	}

	fn add_network() -> NodeNetwork {
		NodeNetwork {
			inputs: vec![0, 0],
			output: 1,
			nodes: [
				(
					0,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![NodeInput::Network, NodeInput::Network],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::unresolved("cons".into())),
					},
				),
				(
					1,
					DocumentNode {
						name: "add".into(),
						inputs: vec![NodeInput::Node(0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::unresolved("add".into())),
					},
				),
			]
			.into_iter()
			.collect(),
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
						name: "cons".into(),
						inputs: vec![NodeInput::Network, NodeInput::Network],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("cons".into(), ConstructionArgs::Unresolved)),
					},
				),
				(
					2,
					DocumentNode {
						name: "add".into(),
						inputs: vec![NodeInput::Node(1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("add".into(), ConstructionArgs::Unresolved)),
					},
				),
			]
			.into_iter()
			.collect(),
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
					inputs: vec![NodeInput::Network, NodeInput::Value(2_u32.into_any())],
					implementation: DocumentNodeImplementation::Network(add_network()),
				},
			)]
			.into_iter()
			.collect(),
		};
		network.flatten_with_fns(1, |self_id, inner_id| self_id * 10 + inner_id, gen_node_id);
		let flat_network = flat_network();

		println!("{:#?}", network);
		println!("{:#?}", flat_network);
		assert_eq!(flat_network, network);
	}

	#[test]
	fn resolve_proto_node_add() {
		let mut d_node = DocumentNode {
			name: "cons".into(),
			inputs: vec![NodeInput::Network, NodeInput::Node(0)],
			implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::value("cons".into(), ConstructionArgs::Unresolved)),
		};

		d_node.resolve_proto_nodes();
		let reference = DocumentNode {
			name: "cons".into(),
			inputs: vec![],
			implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
				name: "cons".into(),
				input: ProtoNodeInput::Network,
				construction_args: ConstructionArgs::Nodes(vec![0]),
			}),
		};
		assert_eq!(d_node, reference);
	}

	#[test]
	fn resolve_flatten_add() {
		let construction_network = NodeNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					1,
					DocumentNode {
						name: "Inc".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "id".into(),
							input: ProtoNodeInput::Node(11),
							construction_args: ConstructionArgs::None,
						}),
					},
				),
				(
					10,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "cons".into(),
							input: ProtoNodeInput::Network,
							construction_args: ConstructionArgs::Nodes(vec![14]),
						}),
					},
				),
				(
					11,
					DocumentNode {
						name: "add".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "add".into(),
							input: ProtoNodeInput::Node(10),
							construction_args: ConstructionArgs::None,
						}),
					},
				),
				(
					14,
					DocumentNode {
						name: "Value: 2".into(),
						inputs: vec![],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode {
							name: "value".into(),
							input: ProtoNodeInput::None,
							construction_args: ConstructionArgs::Value(2_u32.into_any()),
						}),
					},
				),
			]
			.into_iter()
			.collect(),
		};
		let mut resolved_network = flat_network();
		resolved_network.resolve_proto_nodes();

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
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::id()),
					},
				),
				(
					10,
					DocumentNode {
						name: "cons".into(),
						inputs: vec![NodeInput::Network, NodeInput::Node(14)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::unresolved("cons".into())),
					},
				),
				(
					14,
					DocumentNode {
						name: "Value: 2".into(),
						inputs: vec![NodeInput::Value(2_u32.into_any())],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::unresolved("value".into())),
					},
				),
				(
					11,
					DocumentNode {
						name: "add".into(),
						inputs: vec![NodeInput::Node(10)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNode::unresolved("add".into())),
					},
				),
			]
			.into_iter()
			.collect(),
		}
	}
}
