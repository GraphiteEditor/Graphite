use super::mover::Mover;
use crate::NodeIndex;
use glam::IVec2;

impl Mover {
	pub(super) fn shift_step(&mut self, dragged: &[NodeIndex], shift: IVec2, keep_upstream: bool) {
		let before = keep_upstream.then(|| self.snapshot());
		self.shift_nodes(dragged, shift);
		if let Some(before) = before {
			self.restore_followers(&before, dragged);
		}
	}

	fn shift_nodes(&mut self, nodes: &[NodeIndex], shift: IVec2) {
		let mut shifted = Vec::new();
		self.moving_on_their_own(nodes).into_iter().for_each(|node| self.shift_node(node, shift, &mut shifted));
	}

	fn shift_node(&mut self, node: NodeIndex, shift: IVec2, shifted: &mut Vec<NodeIndex>) {
		let moving = self.node_moving_for(node, shift);
		if shifted.contains(&moving) {
			return;
		}
		shifted.push(moving);

		if moving != node {
			self.hold(&[moving]);
		}
		let position = self.position(moving);
		self.place_at(moving, position + shift);
	}

	fn node_moving_for(&mut self, node: NodeIndex, shift: IVec2) -> NodeIndex {
		let (stack, layout) = self.stack_and_layout();
		match shift {
			IVec2 { x: 0, y: -1 } => stack.lowest_layer_with_room_above(layout, node),
			IVec2 { x: 0, .. } => node,
			_ => stack.top(node),
		}
	}

	fn moving_on_their_own(&mut self, nodes: &[NodeIndex]) -> Vec<NodeIndex> {
		let layout = self.layout();
		nodes.iter().copied().filter(|node| !layout.owner_chain(*node).any(|owner| nodes.contains(&owner))).collect()
	}
}
