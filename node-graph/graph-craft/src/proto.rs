use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::document::value;
use crate::document::NodeId;

#[macro_export]
macro_rules! concrete {
	($type:expr) => {
		Type::Concrete(std::borrow::Cow::Borrowed($type))
	};
}
#[macro_export]
macro_rules! generic {
	($type:expr) => {
		Type::Generic(std::borrow::Cow::Borrowed($type))
	};
}

#[derive(Clone, Debug, PartialEq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeIdentifier {
	pub name: std::borrow::Cow<'static, str>,
	pub types: std::borrow::Cow<'static, [Type]>,
}

impl NodeIdentifier {
	pub fn fully_qualified_name(&self) -> String {
		let mut name = String::new();
		name.push_str(self.name.as_ref());
		name.push('<');
		for t in self.types.as_ref() {
			name.push_str(t.to_string().as_str());
			name.push_str(", ");
		}
		name.pop();
		name.pop();
		name.push('>');
		name
	}
}

#[derive(Clone, Debug, PartialEq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
	Generic(std::borrow::Cow<'static, str>),
	Concrete(std::borrow::Cow<'static, str>),
}

impl From<&'static str> for Type {
	fn from(s: &'static str) -> Self {
		Type::Concrete(std::borrow::Cow::Borrowed(s))
	}
}
impl std::fmt::Display for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Type::Generic(name) => write!(f, "{}", name),
			Type::Concrete(name) => write!(f, "{}", name),
		}
	}
}

impl Type {
	pub const fn from_str(concrete: &'static str) -> Self {
		Type::Concrete(std::borrow::Cow::Borrowed(concrete))
	}
}

impl From<&'static str> for NodeIdentifier {
	fn from(s: &'static str) -> Self {
		NodeIdentifier {
			name: std::borrow::Cow::Borrowed(s),
			types: std::borrow::Cow::Borrowed(&[]),
		}
	}
}

impl NodeIdentifier {
	pub const fn new(name: &'static str, types: &'static [Type]) -> Self {
		NodeIdentifier {
			name: std::borrow::Cow::Borrowed(name),
			types: std::borrow::Cow::Borrowed(types),
		}
	}
}

#[derive(Debug, Default, PartialEq)]
pub struct ProtoNetwork {
	// Should a proto Network even allow inputs? Don't think so
	pub inputs: Vec<NodeId>,
	pub output: NodeId,
	pub nodes: Vec<(NodeId, ProtoNode)>,
}

#[derive(Debug)]
pub enum ConstructionArgs {
	Value(value::TaggedValue),
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

impl Hash for ConstructionArgs {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		match self {
			Self::Nodes(nodes) => {
				"nodes".hash(state);
				for node in nodes {
					node.hash(state);
				}
			}
			Self::Value(value) => value.hash(state),
		}
	}
}

impl ConstructionArgs {
	pub fn new_function_args(&self) -> Vec<String> {
		match self {
			ConstructionArgs::Nodes(nodes) => nodes.iter().map(|n| format!("n{}", n)).collect(),
			ConstructionArgs::Value(value) => vec![format!("{:?}", value)],
		}
	}
}

#[derive(Debug, PartialEq)]
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub input: ProtoNodeInput,
	pub identifier: NodeIdentifier,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum ProtoNodeInput {
	None,
	#[default]
	Network,
	Node(NodeId),
}

impl ProtoNodeInput {
	pub fn unwrap_node(self) -> NodeId {
		match self {
			ProtoNodeInput::Node(id) => id,
			_ => panic!("tried to unwrap id from non node input \n node: {:#?}", self),
		}
	}
}

impl ProtoNode {
	pub fn stable_node_id(&self) -> Option<NodeId> {
		use std::hash::Hasher;
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		self.identifier.fully_qualified_name().hash(&mut hasher);
		self.construction_args.hash(&mut hasher);
		match self.input {
			ProtoNodeInput::None => "none".hash(&mut hasher),
			ProtoNodeInput::Network => "network".hash(&mut hasher),
			ProtoNodeInput::Node(id) => id.hash(&mut hasher),
		};
		Some(hasher.finish() as NodeId)
	}

	pub fn value(value: ConstructionArgs) -> Self {
		Self {
			identifier: NodeIdentifier::new("graphene_core::value::ValueNode", &[Type::Generic(Cow::Borrowed("T"))]),
			construction_args: value,
			input: ProtoNodeInput::None,
		}
	}

	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let ProtoNodeInput::Node(id) = self.input {
			self.input = ProtoNodeInput::Node(f(id))
		}
		if let ConstructionArgs::Nodes(ids) = &mut self.construction_args {
			ids.iter_mut().for_each(|id| *id = f(*id));
		}
	}

	pub fn unwrap_construction_nodes(&self) -> Vec<NodeId> {
		match &self.construction_args {
			ConstructionArgs::Nodes(nodes) => nodes.clone(),
			_ => panic!("tried to unwrap nodes from non node construction args \n node: {:#?}", self),
		}
	}
}

impl ProtoNetwork {
	fn check_ref(&self, ref_id: &NodeId, id: &NodeId) {
		assert!(
			self.nodes.iter().any(|(check_id, _)| check_id == ref_id),
			"Node id:{} has a reference which uses node id:{} which doesn't exist in network {:#?}",
			id,
			ref_id,
			self
		);
	}

	pub fn collect_outwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ProtoNodeInput::Node(ref_id) = &node.input {
				self.check_ref(ref_id, id);
				edges.entry(*ref_id).or_default().push(*id)
			}
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*ref_id).or_default().push(*id)
				}
			}
		}
		edges
	}

	pub fn generate_stable_node_ids(&mut self) {
		let mut lookup = self.nodes.iter().map(|(id, _)| (*id, *id)).collect::<HashMap<_, _>>();
		for (ref mut id, node) in self.nodes.iter_mut() {
			if let Some(sni) = node.stable_node_id() {
				lookup.insert(*id, sni);
				*id = sni;
			} else {
				panic!("failed to generate stable node id for node {:#?}", node);
			}
		}
		self.replace_node_references(&lookup)
	}

	pub fn collect_inwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ProtoNodeInput::Node(ref_id) = &node.input {
				self.check_ref(ref_id, id);
				edges.entry(*id).or_default().push(*ref_id)
			}
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*id).or_default().push(*ref_id)
				}
			}
		}
		edges
	}

	pub fn resolve_inputs(&mut self) {
		while !self.resolve_inputs_impl() {}
	}
	fn resolve_inputs_impl(&mut self) -> bool {
		self.reorder_ids();

		let mut lookup = self.nodes.iter().map(|(id, _)| (*id, *id)).collect::<HashMap<_, _>>();
		let compose_node_id = self.nodes.len() as NodeId;
		let inputs = self.nodes.iter().map(|(_, node)| node.input).collect::<Vec<_>>();

		if let Some((input_node, id, input)) = self.nodes.iter_mut().find_map(|(id, node)| {
			if let ProtoNodeInput::Node(input_node) = node.input {
				node.input = ProtoNodeInput::None;
				let pre_node_input = inputs.get(input_node as usize).expect("input node should exist");
				Some((input_node, *id, *pre_node_input))
			} else {
				None
			}
		}) {
			lookup.insert(id, compose_node_id);
			self.replace_node_references(&lookup);
			self.nodes.push((
				compose_node_id,
				ProtoNode {
					identifier: NodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>", &[generic!("T"), generic!("U")]),
					construction_args: ConstructionArgs::Nodes(vec![input_node, id]),
					input,
				},
			));
			return false;
		}

		true
	}

	// Based on https://en.wikipedia.org/wiki/Topological_sorting#Depth-first_search
	// This approach excludes nodes that are not connected
	pub fn topological_sort(&self) -> Vec<NodeId> {
		let mut sorted = Vec::new();
		let inwards_edges = self.collect_inwards_edges();
		fn visit(node_id: NodeId, temp_marks: &mut HashSet<NodeId>, sorted: &mut Vec<NodeId>, inwards_edges: &HashMap<NodeId, Vec<NodeId>>) {
			if sorted.contains(&node_id) {
				return;
			};
			if temp_marks.contains(&node_id) {
				panic!("Cycle detected");
			}

			if let Some(dependencies) = inwards_edges.get(&node_id) {
				temp_marks.insert(node_id);
				for &dependant in dependencies {
					visit(dependant, temp_marks, sorted, inwards_edges);
				}
				temp_marks.remove(&node_id);
			}
			sorted.push(node_id);
		}
		assert!(self.nodes.iter().any(|(id, _)| *id == self.output), "Output id {} does not exist", self.output);
		visit(self.output, &mut HashSet::new(), &mut sorted, &inwards_edges);

		sorted
	}

	/*// Based on https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm
	pub fn topological_sort(&self) -> Vec<NodeId> {
		let mut sorted = Vec::new();
		let outwards_edges = self.collect_outwards_edges();
		let mut inwards_edges = self.collect_inwards_edges();
		let mut no_incoming_edges: Vec<_> = self.nodes.iter().map(|entry| entry.0).filter(|id| !inwards_edges.contains_key(id)).collect();

		assert_ne!(no_incoming_edges.len(), 0, "Acyclic graphs must have at least one node with no incoming edge");

		while let Some(node_id) = no_incoming_edges.pop() {
			sorted.push(node_id);

			if let Some(outwards_edges) = outwards_edges.get(&node_id) {
				for &ref_id in outwards_edges {
					let dependencies = inwards_edges.get_mut(&ref_id).unwrap();
					dependencies.retain(|&id| id != node_id);
					if dependencies.is_empty() {
						no_incoming_edges.push(ref_id)
					}
				}
			}
		}
		info!("Sorted order {sorted:?}");
		sorted
	}*/

	pub fn reorder_ids(&mut self) {
		let order = self.topological_sort();
		// Map of node ids to indexes (which become the node ids as they are inserted into the borrow stack)
		let lookup: HashMap<_, _> = order.iter().enumerate().map(|(pos, id)| (*id, pos as NodeId)).collect();
		self.nodes = order
			.iter()
			.enumerate()
			.map(|(pos, id)| {
				let node = self.nodes.swap_remove(self.nodes.iter().position(|(test_id, _)| test_id == id).unwrap()).1;
				(pos as NodeId, node)
			})
			.collect();
		self.replace_node_references(&lookup);
		assert_eq!(order.len(), self.nodes.len());
	}

	fn replace_node_references(&mut self, lookup: &HashMap<u64, u64>) {
		self.nodes.iter_mut().for_each(|(_, node)| {
			node.map_ids(|id| *lookup.get(&id).expect("node not found in lookup table"));
		});
		self.inputs = self.inputs.iter().filter_map(|id| lookup.get(id).copied()).collect();
		self.output = *lookup.get(&self.output).unwrap();
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};
	use value::IntoValue;

	#[test]
	fn topological_sort() {
		let construction_network = test_network();
		let sorted = construction_network.topological_sort();

		println!("{:#?}", sorted);
		assert_eq!(sorted, vec![14, 10, 11, 1]);
	}

	#[test]
	fn id_reordering() {
		let mut construction_network = test_network();
		construction_network.reorder_ids();
		let sorted = construction_network.topological_sort();
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(sorted, vec![0, 1, 2, 3]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{:#?}", ids);
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(ids, vec![0, 1, 2, 3]);
	}

	#[test]
	fn id_reordering_idempotent() {
		let mut construction_network = test_network();
		construction_network.reorder_ids();
		construction_network.reorder_ids();
		let sorted = construction_network.topological_sort();
		assert_eq!(sorted, vec![0, 1, 2, 3]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{:#?}", ids);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(ids, vec![0, 1, 2, 3]);
	}

	#[test]
	fn input_resolution() {
		let mut construction_network = test_network();
		construction_network.resolve_inputs();
		println!("{:#?}", construction_network);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(construction_network.nodes.len(), 6);
		assert_eq!(construction_network.nodes[5].1.construction_args, ConstructionArgs::Nodes(vec![3, 4]));
	}

	#[test]
	fn stable_node_id_generation() {
		let mut construction_network = test_network();
		construction_network.reorder_ids();
		construction_network.generate_stable_node_ids();
		construction_network.resolve_inputs();
		construction_network.generate_stable_node_ids();
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		assert_eq!(
			ids,
			vec![
				17495035641492238530,
				5865678846923584030,
				2268573767208263092,
				666021810875792436,
				12110007198416821768,
				6701085244080028535
			]
		);
	}

	fn test_network() -> ProtoNetwork {
		ProtoNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					7,
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(11),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					1,
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(11),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					10,
					ProtoNode {
						identifier: "cons".into(),
						input: ProtoNodeInput::Network,
						construction_args: ConstructionArgs::Nodes(vec![14]),
					},
				),
				(
					11,
					ProtoNode {
						identifier: "add".into(),
						input: ProtoNodeInput::Node(10),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					14,
					ProtoNode {
						identifier: "value".into(),
						input: ProtoNodeInput::None,
						construction_args: ConstructionArgs::Value(value::TaggedValue::U32(2)),
					},
				),
			]
			.into_iter()
			.collect(),
		}
	}
}
