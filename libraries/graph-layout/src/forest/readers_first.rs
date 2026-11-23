use crate::connections::Connections;
use crate::per_node::PerNode;
use crate::{Graph, NodeIndex};
use std::iter;

pub(super) struct ReadersFirst<'a> {
	graph: &'a Graph,
	connections: &'a Connections,
	readers_left: PerNode<u32>,
	ready: Vec<NodeIndex>,
	visited: PerNode<bool>,
	unvisited_from: u32,
}

impl<'a> ReadersFirst<'a> {
	pub(super) fn new(graph: &'a Graph, connections: &'a Connections) -> Self {
		let readers_left = PerNode::from_fn(graph, |node| if connections.is_root(graph, node) { 0 } else { connections.readers.count(node) });
		let ready = graph.node_indices().rev().filter(|node| readers_left[*node] == 0).collect();

		Self {
			graph,
			connections,
			readers_left,
			ready,
			visited: PerNode::filled(graph, false),
			unvisited_from: 0,
		}
	}

	fn next_ready(&mut self) -> Option<NodeIndex> {
		iter::from_fn(|| self.ready.pop()).find(|node| !self.visited[*node])
	}

	fn first_unvisited(&mut self) -> Option<NodeIndex> {
		let node = self.visited.indices().skip(self.unvisited_from as usize).find(|node| !self.visited[*node])?;
		self.unvisited_from = node.into();
		Some(node)
	}

	fn release_sources(&mut self, node: NodeIndex) {
		let (graph, connections) = (self.graph, self.connections);
		for wire in connections.inputs(graph, node) {
			self.release(wire.source);
		}
	}

	fn release(&mut self, source: NodeIndex) {
		if self.readers_left[source] == 0 {
			return;
		}

		self.readers_left[source] -= 1;
		if self.readers_left[source] == 0 {
			self.ready.push(source);
		}
	}
}

impl Iterator for ReadersFirst<'_> {
	type Item = NodeIndex;

	fn next(&mut self) -> Option<NodeIndex> {
		let node = self.next_ready().or_else(|| self.first_unvisited())?;
		self.visited[node] = true;
		self.release_sources(node);
		Some(node)
	}
}
