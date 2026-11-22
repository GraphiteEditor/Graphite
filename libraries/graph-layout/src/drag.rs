use crate::{Graph, GroupedByNode, LAYER_HEIGHT, Layout, Placement};
use glam::IVec2;

/// Rows from the top of a layer to the top of the layer stacked closed up under it.
const CLOSED_UP_STACK_SPACING: i32 = LAYER_HEIGHT + 1;

/// The graph as stored once the `dragged` nodes are moved `offset` grid cells from where `lay_out` draws them in `start`.
pub fn dragged(start: &Graph, dragged: &[u32], offset: IVec2, keep_upstream: bool, lay_out: fn(&Graph) -> Layout) -> Graph {
	Drag::new(start, dragged.to_vec(), keep_upstream, lay_out).moved(offset).0
}

/// Nodes dragged from where `lay_out` draws them in the graph the drag starts from. The dragged nodes are held, so their
/// connections are drawn as stored. With `keep_upstream`, each node only carried along by them stays where it is drawn.
///
/// Where the drag puts every node depends only on the start and the offset. Moving up is taken one row at a time, since
/// each row can close up the stack above, and the graph after each row is kept so a drag moving on or back reuses it.
#[derive(Debug, Clone)]
pub struct Drag {
	dragged: Vec<u32>,
	keep_upstream: bool,
	lay_out: fn(&Graph) -> Layout,
	/// The start with the dragged nodes held, then the graph after each further row moved up.
	rows_up: Vec<Graph>,
}

impl Drag {
	pub fn new(start: &Graph, dragged: Vec<u32>, keep_upstream: bool, lay_out: fn(&Graph) -> Layout) -> Self {
		Self::with_layout(start.clone(), lay_out(start), dragged, keep_upstream, lay_out)
	}

	/// Starts dragging from `start` as `lay_out` already laid it out in `start_layout`, which saves laying it out again.
	pub fn with_layout(start: Graph, start_layout: Layout, dragged: Vec<u32>, keep_upstream: bool, lay_out: fn(&Graph) -> Layout) -> Self {
		let mut mover = Mover::new(start, lay_out);
		mover.layout = Some(start_layout);
		mover.hold(&dragged);
		Self {
			dragged,
			keep_upstream,
			lay_out,
			rows_up: vec![mover.graph],
		}
	}

	/// The graph as stored once the dragged nodes are moved `offset` grid cells, and its layout.
	pub fn moved(&mut self, offset: IVec2) -> (Graph, Layout) {
		let rows_up = (-offset.y).max(0) as usize;
		while self.rows_up.len() <= rows_up {
			let mut mover = Mover::new(self.rows_up[self.rows_up.len() - 1].clone(), self.lay_out);
			mover.shift_step(&self.dragged, IVec2::NEG_Y, self.keep_upstream);
			self.rows_up.push(mover.graph);
		}

		// Moving down or sideways moves the same nodes however far it goes, so each is one step
		let mut mover = Mover::new(self.rows_up[rows_up].clone(), self.lay_out);
		for shift in [IVec2::new(0, offset.y.max(0)), IVec2::new(offset.x, 0)] {
			if shift != IVec2::ZERO {
				mover.shift_step(&self.dragged, shift, self.keep_upstream);
			}
		}

		mover.into_graph_and_layout()
	}
}

/// A graph being moved, laid out again whenever a placement changes.
struct Mover {
	graph: Graph,
	readers: GroupedByNode,
	lay_out: fn(&Graph) -> Layout,
	layout: Option<Layout>,
}

impl Mover {
	fn new(graph: Graph, lay_out: fn(&Graph) -> Layout) -> Self {
		let readers = GroupedByNode::grouped(graph.nodes.len(), &graph.wires, |wire| wire.source);
		Self {
			graph,
			readers,
			lay_out,
			layout: None,
		}
	}

	fn into_graph_and_layout(self) -> (Graph, Layout) {
		let layout = self.layout.unwrap_or_else(|| (self.lay_out)(&self.graph));
		(self.graph, layout)
	}

	fn layout(&mut self) -> &Layout {
		self.layout.get_or_insert_with(|| (self.lay_out)(&self.graph))
	}

	fn position(&mut self, node: u32) -> IVec2 {
		self.layout().positions[node as usize]
	}

	fn owner(&mut self, node: u32) -> Option<u32> {
		self.layout().owners[node as usize]
	}

	fn set_placement(&mut self, node: u32, placement: Placement) {
		let stored = &mut self.graph.nodes[node as usize].placement;
		if *stored != placement {
			*stored = placement;
			self.layout = None;
		}
	}

	fn is_relative(&self, node: u32) -> bool {
		matches!(self.graph.nodes[node as usize].placement, Placement::Relative(_))
	}

	/// Holds `nodes`, first storing each one starting to be held, and each node it owns, where it is drawn, so holding moves nothing.
	fn hold(&mut self, nodes: &[u32]) {
		let newly_held: Vec<u32> = nodes.iter().copied().filter(|node| !self.graph.nodes[*node as usize].held).collect();
		if newly_held.is_empty() {
			return;
		}

		let owned_by_newly_held: Vec<u32> = (0..self.graph.nodes.len() as u32)
			.filter(|node| self.owner(*node).is_some_and(|owner| newly_held.contains(&owner)))
			.collect();
		let drawn_before: Vec<(u32, IVec2)> = newly_held.iter().chain(&owned_by_newly_held).map(|node| (*node, self.position(*node))).collect();

		for node in &newly_held {
			self.graph.nodes[*node as usize].held = true;
		}
		self.layout = None;

		// Each owner comes before the nodes it owns, so it is in place before they are stored from it
		for (node, position) in drawn_before {
			if self.position(node) != position {
				self.place_at(node, position);
			}
		}
	}

	/// Stores `position` as an offset from the node's anchor, or as an absolute position if no node reads it.
	fn place_at(&mut self, node: u32, position: IVec2) {
		if self.readers.of(node as usize).is_empty() {
			self.set_placement(node, Placement::Absolute(position));
			return;
		}

		// An absolute node starts a tree of its own, so it only gets an anchor once it is relative
		if !self.is_relative(node) {
			self.set_placement(node, Placement::Relative(IVec2::ZERO));
		}
		let anchor = self.layout().anchors[node as usize];
		self.set_placement(node, Placement::Relative(position - anchor));
	}

	/// Moves the dragged nodes by `shift`, then with `keep_upstream` moves back each node they only carried along.
	fn shift_step(&mut self, dragged: &[u32], shift: IVec2, keep_upstream: bool) {
		let before = keep_upstream.then(|| (self.graph.nodes.iter().map(|node| node.placement).collect::<Vec<_>>(), self.layout().positions.clone()));

		self.shift_nodes(dragged, shift);

		if let Some((placements_before, positions_before)) = before {
			self.restore_followers(&placements_before, &positions_before, dragged);
		}
	}

	/// Shifts each node from where it is drawn, carrying everything placed relative to it. Moving up is one row at a time.
	fn shift_nodes(&mut self, nodes: &[u32], shift: IVec2) {
		let mut shifted = Vec::new();
		for node in self.nodes_to_shift(nodes) {
			// A stack keeps its layers aligned, so sideways it moves as a whole from its top layer
			let moved_node = match shift {
				IVec2 { x: 0, y: -1 } => self.lowest_layer_with_room_above(node),
				IVec2 { x: 0, .. } => node,
				_ => self.stack_top(node),
			};
			if shifted.contains(&moved_node) {
				continue;
			}
			shifted.push(moved_node);

			// A layer moved up in place of the dragged one is held along with it, so it keeps where the drag puts it
			if moved_node != node {
				self.hold(&[moved_node]);
			}
			let position = self.position(moved_node);
			self.place_at(moved_node, position + shift);
		}
	}

	/// The nodes a shift should move on their own, leaving out each one owned, directly or not, by another node being moved, which already carries it along.
	fn nodes_to_shift(&mut self, nodes: &[u32]) -> Vec<u32> {
		let mut to_shift = Vec::new();
		for &node in nodes {
			let mut owner = self.owner(node);
			let mut carried = false;
			while let Some(current) = owner {
				if nodes.contains(&current) {
					carried = true;
					break;
				}
				owner = self.owner(current);
			}
			if !carried {
				to_shift.push(node);
			}
		}
		to_shift
	}

	/// The first layer from `node` up its stack with room to move up before closing up under the layer above it,
	/// or the top layer when the stack is closed up all the way.
	fn lowest_layer_with_room_above(&mut self, node: u32) -> u32 {
		let mut layer = node;
		let mut visited = vec![false; self.graph.nodes.len()];
		while !std::mem::replace(&mut visited[layer as usize], true)
			&& let Some(above) = self.layer_above(layer)
			&& self.position(layer).y - self.position(above).y <= CLOSED_UP_STACK_SPACING
		{
			layer = above;
		}
		layer
	}

	/// The top layer of the stack `node` is stacked in, or the node itself when it is not stacked.
	fn stack_top(&self, node: u32) -> u32 {
		let mut top = node;
		let mut visited = vec![false; self.graph.nodes.len()];
		while !std::mem::replace(&mut visited[top as usize], true)
			&& let Some(above) = self.layer_above(top)
		{
			top = above;
		}
		top
	}

	/// The layer `node` is stacked under, if it is stacked.
	fn layer_above(&self, node: u32) -> Option<u32> {
		let stored = &self.graph.nodes[node as usize];
		let placed_in_a_stack = match stored.placement {
			Placement::Stack(_) => true,
			Placement::Relative(offset) => offset.x == 0,
			Placement::Absolute(_) | Placement::Chain => false,
		};
		if !stored.is_layer || !placed_in_a_stack {
			return None;
		}

		let mut primary_output_readers = self.readers.of(node as usize).iter().map(|wire| self.graph.wires[*wire as usize]).filter(|wire| wire.output == 0);
		let (Some(reader), None) = (primary_output_readers.next(), primary_output_readers.next()) else {
			return None;
		};
		(reader.input == 0 && self.graph.nodes[reader.target as usize].is_layer).then_some(reader.target)
	}

	/// Moves each relative node that was only carried along by the nodes reading it back to where it was drawn, which keeps its upstream in place too.
	/// A node the shift made relative, such as a chain leaving its layer, counts as carried along, while a relative node the shift pushed does not.
	fn restore_followers(&mut self, placements_before: &[Placement], positions_before: &[IVec2], moved_nodes: &[u32]) {
		let followers: Vec<bool> = (0..self.graph.nodes.len() as u32)
			.map(|node| {
				let placement_before = placements_before[node as usize];
				let placement_unchanged = self.graph.nodes[node as usize].placement == placement_before;
				let was_relative = matches!(placement_before, Placement::Relative(_));
				!moved_nodes.contains(&node) && self.is_relative(node) && (placement_unchanged || !was_relative) && self.position(node) != positions_before[node as usize]
			})
			.collect();

		let mut restored = vec![false; self.graph.nodes.len()];
		for node in 0..self.graph.nodes.len() as u32 {
			if followers[node as usize] {
				self.restore_follower(node, &followers, positions_before, &mut restored);
			}
		}
	}

	/// Restores the followers reading `node` first, since each one moves everything it reads.
	fn restore_follower(&mut self, node: u32, followers: &[bool], positions_before: &[IVec2], restored: &mut [bool]) {
		if std::mem::replace(&mut restored[node as usize], true) {
			return;
		}

		let readers: Vec<u32> = self.readers.of(node as usize).iter().map(|wire| self.graph.wires[*wire as usize].target).collect();
		for reader in readers {
			if followers[reader as usize] {
				self.restore_follower(reader, followers, positions_before, restored);
			}
		}

		let shift = positions_before[node as usize] - self.position(node);
		let shifted = match self.graph.nodes[node as usize].placement {
			Placement::Absolute(position) => Placement::Absolute(position + shift),
			Placement::Relative(offset) => Placement::Relative(offset + shift),
			Placement::Chain | Placement::Stack(_) => return,
		};
		self.set_placement(node, shifted);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Node, Wire, layout};

	fn layer(placement: Placement) -> Node {
		Node {
			placement,
			is_layer: true,
			rows: 1,
			width: 5,
			inputs: 2,
			outputs: 1,
			chainable: true,
			waypoint: false,
			held: false,
			name_width: 60.,
			locked: false,
		}
	}

	/// Three layers stacked closed up under a layer at (30, 10).
	fn closed_up_stack() -> Graph {
		let stacked = |source, target| Wire { source, output: 0, target, input: 0 };
		Graph {
			nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), layer(Placement::Stack(0)), layer(Placement::Stack(0))],
			wires: vec![stacked(1, 0), stacked(2, 1)],
		}
	}

	fn positions(graph: &Graph) -> Vec<IVec2> {
		layout(graph).positions
	}

	#[test]
	fn a_drag_back_to_where_it_started_leaves_everything_where_it_was_drawn() {
		let start = closed_up_stack();
		let returned = dragged(&start, &[2], IVec2::ZERO, false, layout);
		assert_eq!(positions(&returned), positions(&start));
	}

	#[test]
	fn a_layer_dragged_up_against_the_layer_above_carries_it_up() {
		let start = closed_up_stack();
		let before = positions(&start);

		let moved_up = positions(&dragged(&start, &[2], IVec2::new(0, -2), false, layout));

		assert_eq!(moved_up[2], before[2] + IVec2::new(0, -2));
		assert_eq!(moved_up[1], before[1] + IVec2::new(0, -2));
	}

	#[test]
	fn a_drag_below_its_start_leaves_the_layers_above_in_place() {
		let start = closed_up_stack();
		let before = positions(&start);

		let moved_back = positions(&dragged(&start, &[2], IVec2::new(0, 1), false, layout));

		assert_eq!(moved_back[2], before[2] + IVec2::new(0, 1));
		assert_eq!(moved_back[1], before[1]);
		assert_eq!(moved_back[0], before[0]);
	}
}
