use super::mover::Mover;
use crate::per_node::PerNode;
use crate::{NodeIndex, Placement};
use glam::IVec2;
use std::mem;

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

	pub(super) fn restore_followers(&mut self, before: &Snapshot, moved: &[NodeIndex]) {
		let followers = self.followers(before, moved);
		let mut restored = PerNode::filled(&self.graph, false);
		followers
			.indices()
			.filter(|node| followers[*node])
			.for_each(|node| self.restore_follower(node, &followers, &before.positions, &mut restored));
	}

	fn followers(&mut self, before: &Snapshot, moved: &[NodeIndex]) -> PerNode<bool> {
		let (graph, layout) = self.graph_and_layout();
		PerNode::from_fn(graph, |node| {
			let placement = graph[node].placement;
			let unchanged = placement == before.placements[node];
			let was_relative = matches!(before.placements[node], Placement::Relative(_));
			let is_relative = matches!(placement, Placement::Relative(_));
			!moved.contains(&node) && is_relative && (unchanged || !was_relative) && layout.positions[node] != before.positions[node]
		})
	}

	fn restore_follower(&mut self, node: NodeIndex, followers: &PerNode<bool>, positions_before: &PerNode<IVec2>, restored: &mut PerNode<bool>) {
		if mem::replace(&mut restored[node], true) {
			return;
		}

		let following_readers: Vec<NodeIndex> = self.readers.of(&self.graph, node).map(|wire| wire.target).filter(|reader| followers[*reader]).collect();
		for reader in following_readers {
			self.restore_follower(reader, followers, positions_before, restored);
		}

		let shift = positions_before[node] - self.position(node);
		if let Some(shifted) = self.graph[node].placement.shifted(shift) {
			self.set_placement(node, shifted);
		}
	}
}
