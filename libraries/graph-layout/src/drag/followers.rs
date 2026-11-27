use super::mover::Mover;
use crate::graph::LAYER_BOTTOM_GAP;
use crate::per_node::PerNode;
use crate::{NodeIndex, Placement};
use glam::IVec2;

pub(super) struct Snapshot {
	placements: PerNode<Placement>,
	positions: PerNode<IVec2>,
}

impl Mover {
	pub(super) fn snapshot(&mut self) -> Snapshot {
		let positions = self.layout().positions.clone();
		let placements = PerNode::from_fn(&self.graph, |node| self.graph[node].placement);
		Snapshot { placements, positions }
	}

	pub(super) fn restore_followers(&mut self, before: &Snapshot, kept: &PerNode<bool>) {
		let followers = self.followers(before, kept);
		let mut pushed = PerNode::filled(&self.graph, None);
		for node in followers.indices().filter(|node| followers[*node]) {
			self.restore_follower(node, &followers, &before.positions, &mut pushed);
		}
	}

	fn followers(&mut self, before: &Snapshot, kept: &PerNode<bool>) -> PerNode<bool> {
		let (graph, layout) = self.graph_and_layout();
		PerNode::from_fn(graph, |node| {
			let placement = graph[node].placement;
			let unchanged = placement == before.placements[node];
			let was_relative = matches!(before.placements[node], Placement::Relative(_));
			let is_relative = matches!(placement, Placement::Relative(_));
			kept[node] && is_relative && (unchanged || !was_relative) && layout.positions[node] != before.positions[node]
		})
	}

	fn restore_follower(&mut self, node: NodeIndex, followers: &PerNode<bool>, positions_before: &PerNode<IVec2>, pushed: &mut PerNode<Option<IVec2>>) -> IVec2 {
		if let Some(push) = pushed[node] {
			return push;
		}
		pushed[node] = Some(IVec2::ZERO);

		let following_readers: Vec<NodeIndex> = self.readers.of(&self.graph, node).map(|wire| wire.target).filter(|reader| followers[*reader]).collect();
		let pushed_by_readers = following_readers
			.into_iter()
			.map(|reader| self.restore_follower(reader, followers, positions_before, pushed))
			.fold(IVec2::ZERO, IVec2::max);

		let shift = positions_before[node] + pushed_by_readers - self.position(node);
		let shifted = self.graph[node].placement.shifted(shift);
		let clear = self.clear_of_the_layer_above(node, shifted);
		self.set_placement(node, clear);

		let push = pushed_by_readers + clear.stored_offset() - shifted.stored_offset();
		pushed[node] = Some(push);
		push
	}

	pub(super) fn carry_along(&mut self, before: &Snapshot, carried: &PerNode<bool>) {
		let mut shifts = PerNode::filled(&self.graph, None);
		for node in carried.indices().filter(|node| carried[*node]) {
			self.carry(node, carried, &before.positions, &mut shifts);
		}
	}

	fn carry(&mut self, node: NodeIndex, carried: &PerNode<bool>, positions_before: &PerNode<IVec2>, shifts: &mut PerNode<Option<IVec2>>) -> IVec2 {
		if let Some(shift) = shifts[node] {
			return shift;
		}
		shifts[node] = Some(IVec2::ZERO);

		let readers: Vec<NodeIndex> = self.readers.of(&self.graph, node).map(|wire| wire.target).collect();
		let mut shift = IVec2::ZERO;
		for reader in readers {
			shift = if carried[reader] {
				self.carry(reader, carried, positions_before, shifts)
			} else {
				self.position(reader) - positions_before[reader]
			};
			if shift != IVec2::ZERO {
				break;
			}
		}

		let shifted = self.graph[node].placement.shifted(shift);
		self.set_placement(node, shifted);
		shifts[node] = Some(shift);
		shift
	}

	fn clear_of_the_layer_above(&self, node: NodeIndex, placement: Placement) -> Placement {
		let Placement::Relative(offset) = placement else { return placement };
		let layer_above = self.readers.of(&self.graph, node).find(|wire| self.graph.feeds_stack(wire)).map(|wire| wire.target);
		let Some(layer_above) = layer_above.filter(|_| offset.x == 0) else { return placement };

		let lowest_free_row = self.graph[layer_above].size.y + LAYER_BOTTOM_GAP;
		Placement::Relative(IVec2::new(0, offset.y.max(lowest_free_row)))
	}
}
