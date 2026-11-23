use crate::connections::Connections;
use crate::forest::Forest;
use crate::graph::CHAIN_OFFSET;
use crate::per_node::PerNode;
use crate::{CHAIN_WIDTH, Graph, NodeIndex};
use glam::IVec2;
use std::iter;

pub(crate) type Beside = fn(IVec2, IVec2) -> bool;

/// Whether a node drawn at `position` joins the chain of the node reading it at `reader_position` once a drag releases it.
#[must_use]
pub fn snaps_into_chain(position: IVec2, reader_position: IVec2) -> bool {
	position.y == reader_position.y && (reader_position.x - CHAIN_WIDTH - 2..=reader_position.x).contains(&position.x)
}

pub(crate) fn sits_in_chain(position: IVec2, reader_position: IVec2) -> bool {
	position == reader_position + CHAIN_OFFSET
}

pub(crate) fn chain_links(graph: &Graph, forest: &Forest) -> PerNode<bool> {
	let mut links = PerNode::filled(graph, false);
	forest
		.order
		.iter()
		.for_each(|node| links[*node] = forest.owner(*node).is_some_and(|owner| is_chain_link(graph, forest, &links, *node, owner)));
	links
}

fn is_chain_link(graph: &Graph, forest: &Forest, links: &PerNode<bool>, node: NodeIndex, owner: NodeIndex) -> bool {
	let feeds_chain_input = forest.owner_inputs[node] == graph[owner].chain_input();
	let owner_in_chain = graph[owner].is_layer() || links[owner];
	graph[node].can_join_chain() && feeds_chain_input && forest.offsets[node] == CHAIN_OFFSET && owner_in_chain
}

pub(crate) struct Chains<'a> {
	pub(crate) graph: &'a Graph,
	pub(crate) connections: &'a Connections,
	pub(crate) positions: &'a PerNode<IVec2>,
	pub(crate) beside: Beside,
}

impl Chains<'_> {
	pub(crate) fn layers(&self) -> PerNode<Option<NodeIndex>> {
		let mut layers = PerNode::filled(self.graph, None);
		self.graph
			.node_indices()
			.filter(|node| self.graph[*node].is_layer())
			.flat_map(|layer| self.links(layer).map(move |link| (link, layer)))
			.for_each(|(link, layer)| layers[link] = Some(layer));
		layers
	}

	fn links(&self, layer: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
		iter::successors(Some(layer), |reader| self.next_link(*reader)).skip(1)
	}

	fn next_link(&self, reader: NodeIndex) -> Option<NodeIndex> {
		let wire = self.connections.inputs(self.graph, reader).find(|wire| wire.input == self.graph[reader].chain_input())?;
		let source = wire.source;
		let read_once = self.connections.readers(self.graph, source).filter(|wire| wire.is_primary()).count() == 1;
		let beside = (self.beside)(self.positions[source], self.positions[reader]);
		(self.graph[source].can_join_chain() && wire.is_primary() && read_once && beside).then_some(source)
	}
}

pub(crate) fn chain_lengths(graph: &Graph, chain_layers: &PerNode<Option<NodeIndex>>) -> PerNode<u32> {
	let mut lengths = PerNode::filled(graph, 0);
	chain_layers.iter().flatten().for_each(|layer| lengths[*layer] += 1);
	lengths
}
