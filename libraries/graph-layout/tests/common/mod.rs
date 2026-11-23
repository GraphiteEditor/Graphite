#![allow(dead_code, reason = "Each test file uses a different part of the shared builders")]

use glam::IVec2;
use graph_layout::{Graph, Layout, Node, NodeIndex, NodeKind, Placement, Wire};

pub fn node(placement: Placement) -> Node {
	Node {
		placement,
		kind: NodeKind::Node,
		size: IVec2::new(5, 2),
		chainable: true,
		held: false,
	}
}

pub fn tall_node(placement: Placement, rows: i32) -> Node {
	Node {
		size: IVec2::new(5, rows + 1),
		..node(placement)
	}
}

pub fn layer(placement: Placement) -> Node {
	Node {
		kind: NodeKind::Layer,
		..node(placement)
	}
}

pub fn waypoint(placement: Placement) -> Node {
	Node {
		kind: NodeKind::Waypoint,
		chainable: false,
		..node(placement)
	}
}

pub fn wire(source: u32, target: u32, input: u32) -> Wire {
	Wire {
		source: source.into(),
		output: 0,
		target: target.into(),
		input,
	}
}

pub fn at(index: u32) -> NodeIndex {
	index.into()
}

pub fn chain_layers(graph: &Graph, layout: &Layout) -> Vec<Option<u32>> {
	let count = u32::try_from(graph.nodes.len()).unwrap_or_default();
	(0..count).map(|index| layout.node(at(index)).chain_layer.map(u32::from)).collect()
}
