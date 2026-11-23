use super::stack::Stack;
use crate::connections::WiresByNode;
use crate::{Graph, Layout, NodeIndex, Placement};
use glam::IVec2;

pub(super) struct Mover {
	pub(super) graph: Graph,
	pub(super) readers: WiresByNode,
	layout: Option<Layout>,
}

impl Mover {
	pub(super) fn new(graph: Graph, layout: Option<Layout>) -> Self {
		let readers = WiresByNode::readers(&graph);
		Self { graph, readers, layout }
	}

	pub(super) fn into_graph(self) -> Graph {
		self.graph
	}

	pub(super) fn into_graph_and_layout(self) -> (Graph, Layout) {
		let layout = self.layout.unwrap_or_else(|| Layout::new(&self.graph));
		(self.graph, layout)
	}

	pub(super) fn layout(&mut self) -> &Layout {
		self.layout.get_or_insert_with(|| Layout::new(&self.graph))
	}

	pub(super) fn graph_and_layout(&mut self) -> (&Graph, &Layout) {
		let layout = self.layout.get_or_insert_with(|| Layout::new(&self.graph));
		(&self.graph, layout)
	}

	pub(super) fn stack_and_layout(&mut self) -> (Stack<'_>, &Layout) {
		let layout = self.layout.get_or_insert_with(|| Layout::new(&self.graph));
		let stack = Stack {
			graph: &self.graph,
			readers: &self.readers,
		};
		(stack, layout)
	}

	pub(super) fn position(&mut self, node: NodeIndex) -> IVec2 {
		self.layout().positions[node]
	}

	pub(super) fn set_placement(&mut self, node: NodeIndex, placement: Placement) {
		if self.graph[node].placement != placement {
			self.graph[node].placement = placement;
			self.layout = None;
		}
	}

	pub(super) fn hold(&mut self, nodes: &[NodeIndex]) {
		let newly_held: Vec<NodeIndex> = nodes.iter().copied().filter(|node| !self.graph[*node].held).collect();
		if newly_held.is_empty() {
			return;
		}

		let layout = self.layout();
		let owned: Vec<NodeIndex> = layout.owners.indices().filter(|node| layout.owners[*node].is_some_and(|owner| newly_held.contains(&owner))).collect();
		let mut carriers: Vec<NodeIndex> = newly_held.iter().flat_map(|node| layout.owner_chain(*node)).filter(|carrier| !newly_held.contains(carrier)).collect();
		carriers.sort_by_key(|carrier| (layout.owner_chain(*carrier).count(), *carrier));
		carriers.dedup();
		let drawn_before: Vec<(NodeIndex, IVec2)> = carriers.iter().chain(&newly_held).chain(&owned).map(|node| (*node, layout.positions[*node])).collect();
		let holding_moves_something = newly_held.iter().chain(&owned).chain(&carriers).any(|node| layout.elongations[*node] != IVec2::ZERO);

		for node in &newly_held {
			self.graph[*node].held = true;
		}
		if holding_moves_something {
			self.layout = None;
			for (node, position) in drawn_before {
				self.keep_drawn_at(node, position);
			}
		}
	}

	pub(super) fn place_at(&mut self, node: NodeIndex, position: IVec2) {
		if self.readers.count(node) == 0 {
			self.set_placement(node, Placement::Absolute(position));
			return;
		}

		if !matches!(self.graph[node].placement, Placement::Relative(_)) {
			self.set_placement(node, Placement::Relative(IVec2::ZERO));
		}
		let anchor = self.layout().anchors[node];
		self.set_placement(node, Placement::Relative(position - anchor));
	}

	fn keep_drawn_at(&mut self, node: NodeIndex, position: IVec2) {
		if self.position(node) != position {
			self.place_at(node, position);
		}
	}
}
