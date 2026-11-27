use crate::connections::WiresByNode;
use crate::graph::LAYER_BOTTOM_GAP;
use crate::per_node::PerNode;
use crate::{Graph, Layout, NodeIndex, Placement};
use std::{iter, mem};

pub(super) struct Stack<'a> {
	pub(super) graph: &'a Graph,
	pub(super) readers: &'a WiresByNode,
}

impl Stack<'_> {
	pub(super) fn top(&self, node: NodeIndex) -> NodeIndex {
		self.climb(node, |layer| self.layer_above(layer))
	}

	pub(super) fn lowest_layer_with_room_above(&self, layout: &Layout, node: NodeIndex) -> Option<NodeIndex> {
		let top_of_closed_up_run = self.climb(node, |layer| self.layer_above(layer).filter(|above| self.closed_up(layout, layer, *above)));
		(!self.closed_up_under_its_parent(layout, top_of_closed_up_run)).then_some(top_of_closed_up_run)
	}

	fn closed_up_under_its_parent(&self, layout: &Layout, layer: NodeIndex) -> bool {
		let parent = self
			.readers
			.of(self.graph, layer)
			.find(|wire| wire.is_primary() && self.graph[wire.target].is_layer() && !self.graph.feeds_stack(wire))
			.map(|wire| wire.target);
		self.graph[layer].is_layer() && parent.is_some_and(|parent| self.closed_up(layout, layer, parent))
	}

	fn closed_up(&self, layout: &Layout, layer: NodeIndex, above: NodeIndex) -> bool {
		layout.positions[layer].y - layout.positions[above].y <= self.graph[above].size.y + LAYER_BOTTOM_GAP
	}

	fn climb(&self, start: NodeIndex, step: impl Fn(NodeIndex) -> Option<NodeIndex>) -> NodeIndex {
		let mut visited = PerNode::filled(self.graph, false);
		let mut last = start;
		iter::successors(Some(start), |layer| step(*layer))
			.find(|layer| {
				last = *layer;
				mem::replace(&mut visited[*layer], true)
			})
			.unwrap_or(last)
	}

	fn layer_above(&self, node: NodeIndex) -> Option<NodeIndex> {
		let stored = &self.graph[node];
		let placed_in_a_stack = matches!(stored.placement, Placement::Relative(offset) if offset.x == 0);
		if !stored.is_layer() || !placed_in_a_stack {
			return None;
		}

		let mut primary_readers = self.readers.of(self.graph, node).filter(|wire| wire.is_primary());
		let (Some(reader), None) = (primary_readers.next(), primary_readers.next()) else {
			return None;
		};
		self.graph.feeds_stack(reader).then_some(reader.target)
	}
}
