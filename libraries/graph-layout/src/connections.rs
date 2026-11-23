use crate::groups::Groups;
use crate::{Graph, NodeIndex, Placement, Wire};

#[derive(Clone, Debug)]
pub(crate) struct WiresByNode(Groups<u32>);

impl WiresByNode {
	pub(crate) fn readers(graph: &Graph) -> Self {
		Self::grouped(graph, |wire| wire.source)
	}

	pub(crate) fn inputs(graph: &Graph) -> Self {
		Self::grouped(graph, |wire| wire.target)
	}

	fn grouped(graph: &Graph, node_of: impl Fn(&Wire) -> NodeIndex) -> Self {
		let wire_indices = 0..u32::try_from(graph.wires.len()).unwrap_or(u32::MAX);
		Self(Groups::new(graph, wire_indices, |index| node_of(&graph.wires[*index as usize])))
	}

	pub(crate) fn of<'a>(&'a self, graph: &'a Graph, node: NodeIndex) -> impl Iterator<Item = &'a Wire> + Clone {
		self.0.of(node).iter().map(|index| &graph.wires[*index as usize])
	}

	pub(crate) fn count(&self, node: NodeIndex) -> u32 {
		u32::try_from(self.0.of(node).len()).unwrap_or(u32::MAX)
	}
}

pub(crate) struct Connections {
	pub(crate) readers: WiresByNode,
	pub(crate) inputs: WiresByNode,
}

impl Connections {
	pub(crate) fn new(graph: &Graph) -> Self {
		Self {
			readers: WiresByNode::readers(graph),
			inputs: WiresByNode::inputs(graph),
		}
	}

	pub(crate) fn readers<'a>(&'a self, graph: &'a Graph, node: NodeIndex) -> impl Iterator<Item = &'a Wire> + Clone {
		self.readers.of(graph, node)
	}

	pub(crate) fn inputs<'a>(&'a self, graph: &'a Graph, node: NodeIndex) -> impl Iterator<Item = &'a Wire> {
		self.inputs.of(graph, node)
	}

	pub(crate) fn is_root(&self, graph: &Graph, node: NodeIndex) -> bool {
		self.readers.count(node) == 0 || matches!(graph[node].placement, Placement::Absolute(_))
	}
}
