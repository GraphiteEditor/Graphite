use std::collections::HashMap;

use crate::document::value;
use crate::document::NodeId;

#[derive(Clone, Debug, PartialEq)]
pub struct NodeIdentifier<'a> {
	pub name: &'a str,
	pub types: &'a [Type<'a>],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type<'a> {
	Generic,
	Concrete(&'a str),
}

impl<'a> From<&'a str> for Type<'a> {
	fn from(s: &'a str) -> Self {
		Type::Concrete(s)
	}
}
impl<'a> From<&'a str> for NodeIdentifier<'a> {
	fn from(s: &'a str) -> Self {
		NodeIdentifier { name: s, types: &[] }
	}
}

impl<'a> NodeIdentifier<'a> {
	pub fn new(name: &'a str, types: &'a [Type<'a>]) -> Self {
		NodeIdentifier { name, types }
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
	pub identifier: NodeIdentifier<'static>,
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
			identifier: NodeIdentifier {
				name: "graphene_core::value::ValueNode",
				types: &[Type::Generic],
			},
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
	pub fn collect_outwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ProtoNodeInput::Node(ref_id) = &node.input {
				edges.entry(*ref_id).or_default().push(*id)
			}
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
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
				edges.entry(*id).or_default().push(*ref_id)
			}
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					edges.entry(*id).or_default().push(*ref_id)
				}
			}
		}
		edges
	}

	// Based on https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm
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
		sorted
	}

	pub fn reorder_ids(&mut self) {
		let order = self.topological_sort();
		let lookup = self
			.nodes
			.iter()
			.map(|(id, _)| (*id, order.iter().position(|x| x == id).unwrap() as u64))
			.collect::<HashMap<u64, u64>>();
		self.nodes.sort_by_key(|(id, _)| lookup.get(id).unwrap());
		self.nodes.iter_mut().for_each(|(id, node)| {
			node.map_ids(|id| *lookup.get(&id).unwrap());
			*id = *lookup.get(id).unwrap()
		});
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
