mod followers;
mod mover;
mod shift;
mod stack;

use crate::{Graph, Layout, NodeIndex};
use glam::IVec2;
use mover::Mover;

/// Nodes dragged from where they are drawn, computing the graph for any offset from the start alone.
/// Rows moved up are kept, so a drag going on or coming back reuses them.
#[derive(Clone, Debug)]
pub struct Drag {
	dragged: Vec<NodeIndex>,
	keep_upstream: bool,
	rows_up: Vec<Graph>,
}

impl Drag {
	/// Holds the `dragged` nodes of `start`. With `keep_upstream`, nodes only carried along by them stay where they are drawn.
	#[must_use]
	pub fn new(start: Graph, start_layout: Layout, dragged: Vec<NodeIndex>, keep_upstream: bool) -> Self {
		let mut mover = Mover::new(start, Some(start_layout));
		mover.hold(&dragged);
		Self {
			dragged,
			keep_upstream,
			rows_up: vec![mover.into_graph()],
		}
	}

	/// The graph as stored and its layout once the dragged nodes are moved `offset` grid cells.
	pub fn moved(&mut self, offset: IVec2) -> (Graph, Layout) {
		let rows_up = offset.y.min(0).unsigned_abs() as usize;
		self.extend_rows_up_to(rows_up);

		let mut mover = Mover::new(self.rows_up[rows_up].clone(), None);
		[IVec2::new(0, offset.y.max(0)), IVec2::new(offset.x, 0)]
			.into_iter()
			.filter(|shift| *shift != IVec2::ZERO)
			.for_each(|shift| mover.shift_step(&self.dragged, shift, self.keep_upstream));
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
		mover.shift_step(&self.dragged, IVec2::NEG_Y, self.keep_upstream);
		mover.into_graph()
	}
}
