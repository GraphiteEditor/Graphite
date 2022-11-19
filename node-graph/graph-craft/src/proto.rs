use std::collections::{HashMap, HashSet};

use crate::document::value;
use crate::document::NodeId;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeIdentifier {
	pub name: std::borrow::Cow<'static, str>,
	pub types: std::borrow::Cow<'static, [Type]>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
	Generic,
	Concrete(std::borrow::Cow<'static, str>),
}

impl From<&'static str> for Type {
	fn from(s: &'static str) -> Self {
		Type::Concrete(std::borrow::Cow::Borrowed(s))
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
	pub inputs: Vec<NodeId>,
	pub output: NodeId,
	pub nodes: Vec<(NodeId, ProtoNode)>,
}

#[derive(Debug)]
pub enum ConstructionArgs {
	Value(value::Value),
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

#[derive(Debug, PartialEq)]
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub input: ProtoNodeInput,
	pub identifier: NodeIdentifier,
}

#[derive(Debug, Default, PartialEq, Eq)]
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
	pub fn value(value: ConstructionArgs) -> Self {
		Self {
			identifier: NodeIdentifier::new("graphene_core::value::ValueNode", &[Type::Generic]),
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
			info!("Visiting {node_id}");

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

		info!("Sorted order {sorted:?}");
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
		info!("Order {order:?}");
		self.nodes = order
			.iter()
			.map(|id| {
				let mut node = self.nodes.swap_remove(self.nodes.iter().position(|(test_id, _)| test_id == id).unwrap()).1;
				node.map_ids(|id| *lookup.get(&id).unwrap());
				(*lookup.get(id).unwrap(), node)
			})
			.collect();
		assert_eq!(order.len(), self.nodes.len());
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};
	use value::IntoValue;

	#[test]
	fn topological_sort() {
		let construction_network = ProtoNetwork {
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
						construction_args: ConstructionArgs::Value(2_u32.into_any()),
					},
				),
			]
			.into_iter()
			.collect(),
		};
		let sorted = construction_network.topological_sort();

		println!("{:#?}", sorted);
		assert_eq!(sorted, vec![14, 10, 11, 1]);
	}
}
