use crate::chains::{Beside, Chains, chain_lengths, sits_in_chain, snaps_into_chain};
use crate::connections::Connections;
use crate::fitting::elongations;
use crate::forest::Forest;
use crate::per_node::PerNode;
use crate::{CHAIN_WIDTH, Graph, NodeIndex, Rect};
use glam::IVec2;
use std::iter;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Where one node is drawn and how it was placed there.
pub struct NodeLayout {
	/// The top left corner in grid cells.
	pub position: IVec2,
	/// The highest reader as stored, which the node is fitted with and drawn level with.
	pub owner: Option<NodeIndex>,
	/// The point a relative offset is taken from: the left of the node's readers, level with its owner.
	pub anchor: IVec2,
	/// How much further the node is drawn from its owner than stored, so its upstream clears what is placed before it.
	pub elongation: IVec2,
	/// The layer whose chain the node is drawn in.
	pub chain_layer: Option<NodeIndex>,
	/// How many nodes the chain of this layer holds.
	pub chain_length: u32,
}

/// Where every node of a graph is drawn.
#[derive(Clone, Debug, Default)]
pub struct Layout {
	pub(crate) positions: PerNode<IVec2>,
	pub(crate) owners: PerNode<Option<NodeIndex>>,
	pub(crate) anchors: PerNode<IVec2>,
	pub(crate) elongations: PerNode<IVec2>,
	chain_layers: PerNode<Option<NodeIndex>>,
	chain_lengths: PerNode<u32>,
}

impl Layout {
	/// Lays out `graph` in one pass, placing each node after all the nodes reading it.
	#[must_use]
	pub fn new(graph: &Graph) -> Self {
		let connections = Connections::new(graph);
		let forest = Forest::grow(graph, &connections);

		let mut layout = Self {
			positions: PerNode::filled(graph, IVec2::ZERO),
			anchors: PerNode::filled(graph, IVec2::ZERO),
			elongations: elongations(graph, &forest),
			owners: PerNode::default(),
			chain_layers: PerNode::default(),
			chain_lengths: PerNode::default(),
		};
		for node in &forest.order {
			layout.place(graph, &connections, &forest, *node);
		}
		layout.owners = forest.owners;
		layout.find_chains(graph, &connections, sits_in_chain);
		layout
	}

	/// Draws each node in the chain it would join if a drag released it now, rather than only in the chain it sits in.
	pub fn snap_chains(&mut self, graph: &Graph) {
		self.find_chains(graph, &Connections::new(graph), snaps_into_chain);
	}

	#[must_use]
	pub fn node(&self, node: NodeIndex) -> NodeLayout {
		NodeLayout {
			position: self.positions[node],
			owner: self.owners[node],
			anchor: self.anchors[node],
			elongation: self.elongations[node],
			chain_layer: self.chain_layers[node],
			chain_length: self.chain_lengths[node],
		}
	}

	/// A box in grid cells around the node and a layer's chain, for finding which nodes a point or a box may hit.
	#[must_use]
	pub fn reach(&self, graph: &Graph, node: NodeIndex) -> Rect {
		let position = self.positions[node];
		let chain_area = if graph[node].is_layer() {
			IVec2::new(CHAIN_WIDTH * self.chain_lengths[node].cast_signed() + 1, 0)
		} else {
			IVec2::ZERO
		};
		Rect::new(position - chain_area, position + graph[node].size)
	}

	pub(crate) fn owner_chain(&self, node: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
		iter::successors(self.owners[node], |owner| self.owners[*owner])
	}

	fn place(&mut self, graph: &Graph, connections: &Connections, forest: &Forest, node: NodeIndex) {
		self.anchors[node] = forest.anchor(graph, connections, node, |reader| self.positions[reader]);
		self.positions[node] = match forest.owner(node) {
			Some(_) => self.anchors[node] + graph[node].placement.stored_offset() + self.elongations[node],
			None => graph[node].placement.root_position(),
		};
	}

	fn find_chains(&mut self, graph: &Graph, connections: &Connections, beside: Beside) {
		self.chain_layers = Chains {
			graph,
			connections,
			positions: &self.positions,
			beside,
		}
		.layers();
		self.chain_lengths = chain_lengths(graph, &self.chain_layers);
	}
}
