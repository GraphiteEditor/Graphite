use crate::connections::WiresByNode;
use crate::per_node::PerNode;
use crate::{Graph, NodeIndex, Placement, Wire};
use glam::IVec2;
use std::mem;

#[derive(Clone, Debug)]
pub(super) struct UpstreamOfDragged {
	moving_up_or_down: Upstream,
	moving_sideways: Upstream,
}

#[derive(Clone, Debug)]
pub(super) struct Upstream {
	pub(super) kept: PerNode<bool>,
	pub(super) carried: PerNode<bool>,
}

impl UpstreamOfDragged {
	pub(super) fn new(graph: &Graph, dragged: &[NodeIndex]) -> Self {
		let inputs = WiresByNode::inputs(graph);
		let everything = upstream_through(graph, &inputs, dragged, |_| true);
		let into_a_node = |wire: &Wire| !graph[wire.target].is_layer();
		let split = |kept: PerNode<bool>| {
			let carried = PerNode::from_fn(graph, |node| everything[node] && !kept[node] && matches!(graph[node].placement, Placement::Absolute(_)));
			Upstream { kept, carried }
		};
		Self {
			moving_up_or_down: split(upstream_through(graph, &inputs, dragged, |wire| into_a_node(wire) || graph.feeds_stack(wire))),
			moving_sideways: split(upstream_through(graph, &inputs, dragged, into_a_node)),
		}
	}

	pub(super) fn moving_by(&self, shift: IVec2) -> &Upstream {
		if shift.x == 0 { &self.moving_up_or_down } else { &self.moving_sideways }
	}
}

fn upstream_through(graph: &Graph, inputs: &WiresByNode, dragged: &[NodeIndex], followed: impl Fn(&Wire) -> bool) -> PerNode<bool> {
	let mut upstream = PerNode::filled(graph, false);
	for node in dragged {
		upstream[*node] = true;
	}
	dragged
		.iter()
		.flat_map(|node| inputs.of(graph, *node))
		.filter(|wire| followed(wire))
		.for_each(|wire| mark_upstream(graph, inputs, wire.source, &mut upstream));
	for node in dragged {
		upstream[*node] = false;
	}
	upstream
}

fn mark_upstream(graph: &Graph, inputs: &WiresByNode, node: NodeIndex, upstream: &mut PerNode<bool>) {
	if mem::replace(&mut upstream[node], true) {
		return;
	}
	inputs.of(graph, node).for_each(|wire| mark_upstream(graph, inputs, wire.source, upstream));
}
