mod common;

use common::{at, chain_layers, layer, node, wire};
use glam::IVec2;
use graph_layout::{CHAIN_WIDTH, Graph, Layout, Placement, snaps_into_chain};

#[test]
fn a_node_beside_a_layer_joins_its_chain_however_it_is_placed() {
	let graph = Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			node(Placement::Absolute(IVec2::new(30 - CHAIN_WIDTH, 10))),
			node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0))),
			node(Placement::Absolute(IVec2::ZERO)),
		],
		wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, 2, 0)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(chain_layers(&graph, &layout), vec![None, Some(0), Some(0), None]);
	assert_eq!(layout.node(at(0)).chain_length, 2);
}

#[test]
fn a_chain_is_never_drawn_apart() {
	let graph = Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			node(Placement::Chain),
			node(Placement::Chain),
			common::tall_node(Placement::Relative(IVec2::new(0, 3)), 3),
		],
		wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, 1, 1)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(1)).elongation, IVec2::ZERO);
	assert_eq!(layout.node(at(2)).elongation, IVec2::ZERO);
	assert_eq!(chain_layers(&graph, &layout), vec![None, Some(0), Some(0), None]);
}

#[test]
fn a_node_dropped_near_a_layer_shows_in_the_chain_it_snaps_into() {
	let graph = Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			node(Placement::Absolute(IVec2::new(30 - CHAIN_WIDTH - 1, 10))),
			node(Placement::Absolute(IVec2::ZERO)),
		],
		wires: vec![wire(1, 0, 1), wire(2, 1, 0)],
	};

	let mut layout = Layout::new(&graph);
	assert_eq!(chain_layers(&graph, &layout), vec![None, None, None]);

	layout.snap_chains(&graph);
	assert_eq!(chain_layers(&graph, &layout), vec![None, Some(0), None]);
	assert_eq!(layout.node(at(0)).chain_length, 1);
}

#[test]
fn a_node_snaps_into_a_chain_from_slightly_left_of_its_slot_up_to_its_reader() {
	let reader = IVec2::new(30, 10);

	assert!(snaps_into_chain(reader - IVec2::new(CHAIN_WIDTH + 2, 0), reader));
	assert!(snaps_into_chain(reader, reader));
	assert!(!snaps_into_chain(reader - IVec2::new(CHAIN_WIDTH + 3, 0), reader));
	assert!(!snaps_into_chain(reader - IVec2::new(CHAIN_WIDTH, 1), reader));
}
