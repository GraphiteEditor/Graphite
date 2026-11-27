mod followers;
mod kept;
mod mover;
mod shift;
mod stack;

use crate::{Graph, Layout, NodeIndex, Placement};
use glam::IVec2;
use kept::UpstreamOfDragged;
use mover::Mover;

/// Nodes dragged from where they are drawn, computing the graph for any offset from the start alone.
/// Rows moved up are kept, so a drag going on or coming back reuses them.
#[derive(Clone, Debug)]
pub struct Drag {
	dragged: Vec<NodeIndex>,
	upstream: Option<UpstreamOfDragged>,
	rows_up: Vec<Graph>,
	held_in_place: Vec<HeldInPlace>,
}

#[derive(Clone, Copy, Debug)]
struct HeldInPlace {
	node: NodeIndex,
	stored: Placement,
	held: Placement,
}

impl Drag {
	/// Holds the `dragged` nodes of `start`. With `keep_upstream`, nodes only carried along by them stay where they are drawn.
	/// Dragged layers still carry their content and chain, and carry the stacks below them when moved sideways.
	/// Nodes starting a tree of their own upstream of what moves follow the nodes reading them.
	#[must_use]
	pub fn new(start: Graph, start_layout: Layout, dragged: Vec<NodeIndex>, keep_upstream: bool) -> Self {
		let upstream = keep_upstream.then(|| UpstreamOfDragged::new(&start, &dragged));
		let stored: Vec<Placement> = start.nodes.iter().map(|node| node.placement).collect();
		let mut mover = Mover::new(start, Some(start_layout));
		mover.hold(&dragged);
		let held = mover.into_graph();

		let held_in_place = held
			.node_indices()
			.zip(stored)
			.filter(|(node, stored)| held[*node].placement != *stored)
			.map(|(node, stored)| HeldInPlace {
				node,
				stored,
				held: held[node].placement,
			})
			.collect();
		Self {
			dragged,
			upstream,
			rows_up: vec![held],
			held_in_place,
		}
	}

	/// The graph to store once the drag ends `offset` grid cells from its start. Nodes the drag only held in place go back to how they were stored.
	pub fn released(&mut self, offset: IVec2) -> Graph {
		let (mut graph, _) = self.moved(offset);
		for node in &mut graph.nodes {
			node.held = false;
		}
		for HeldInPlace { node, stored, held } in &self.held_in_place {
			if graph[*node].placement == *held {
				graph[*node].placement = *stored;
			}
		}
		graph
	}

	/// The graph as stored and its layout once the dragged nodes are moved `offset` grid cells.
	pub fn moved(&mut self, offset: IVec2) -> (Graph, Layout) {
		let rows_up = offset.y.min(0).unsigned_abs() as usize;
		self.extend_rows_up_to(rows_up);

		let mut mover = Mover::new(self.rows_up[rows_up].clone(), None);
		[IVec2::new(0, offset.y.max(0)), IVec2::new(offset.x, 0)]
			.into_iter()
			.filter(|shift| *shift != IVec2::ZERO)
			.for_each(|shift| mover.shift_step(&self.dragged, shift, self.upstream.as_ref().map(|upstream| upstream.moving_by(shift))));
		mover.into_graph_and_layout()
	}

	fn extend_rows_up_to(&mut self, rows: usize) {
		let missing = (rows + 1).saturating_sub(self.rows_up.len());
		(0..missing).for_each(|_| {
			let next_row_up = self.one_row_further_up();
			self.rows_up.push(next_row_up);
		});
	}

	fn one_row_further_up(&self) -> Graph {
		let highest_so_far = self.rows_up.last().cloned().unwrap_or_default();
		let mut mover = Mover::new(highest_so_far, None);
		mover.shift_step(&self.dragged, IVec2::NEG_Y, self.upstream.as_ref().map(|upstream| upstream.moving_by(IVec2::NEG_Y)));
		mover.into_graph()
	}
}
