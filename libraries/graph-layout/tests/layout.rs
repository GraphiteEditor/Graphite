mod common;

use common::{CHAINED, LAYER_HEIGHT, at, chain_layers, layer, node, stacked, tall_node, waypoint, wire};
use glam::IVec2;
use graph_layout::{CHAIN_WIDTH, Graph, Layout, Placement};

const COLUMNS_BETWEEN_NODES: i32 = 2;

fn stack_with_tall_content(tall_node_reader: u32) -> Graph {
	Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			node(CHAINED),
			node(CHAINED),
			tall_node(Placement::Relative(IVec2::new(0, 3)), 3),
			layer(Placement::Relative(IVec2::new(0, LAYER_HEIGHT + 1))),
			node(CHAINED),
		],
		wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, tall_node_reader, 1), wire(4, 0, 0), wire(5, 4, 1)],
	}
}

fn far_upstream_beside_tall_content(also_read_from_beside: bool) -> Graph {
	let far_offset = if also_read_from_beside { IVec2::new(-CHAIN_WIDTH, 30) } else { IVec2::new(-10, 30) };
	let mut wires = vec![wire(1, 0, 0), wire(2, 1, 1), wire(3, 0, 1), wire(5, 3, 0)];
	if also_read_from_beside {
		wires.push(wire(5, 4, 0));
	}

	Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			layer(stacked(0)),
			tall_node(Placement::Relative(IVec2::new(-14, 0)), 30),
			node(CHAINED),
			node(Placement::Absolute(IVec2::new(20, 40))),
			node(Placement::Relative(far_offset)),
		],
		wires,
	}
}

#[test]
fn placements_resolve_from_their_readers() {
	let graph = Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			layer(stacked(1)),
			node(CHAINED),
			node(CHAINED),
			node(Placement::Relative(IVec2::new(-7, 3))),
		],
		wires: vec![wire(1, 0, 0), wire(2, 0, 1), wire(3, 2, 0), wire(4, 2, 1), wire(4, 1, 1)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(1)).position, IVec2::new(30, 10 + LAYER_HEIGHT + 1 + 1));
	assert_eq!(layout.node(at(2)).position, IVec2::new(30 - CHAIN_WIDTH, 10));
	assert_eq!(layout.node(at(3)).position, IVec2::new(30 - 2 * CHAIN_WIDTH, 10));
	assert_eq!(layout.node(at(4)).position, IVec2::new(30 - CHAIN_WIDTH - 7, 10 + 3));
	assert_eq!(chain_layers(&graph, &layout), vec![None, None, Some(0), Some(0), None]);
}

#[test]
fn a_cycle_is_broken_at_its_first_node() {
	let graph = Graph {
		nodes: vec![node(Placement::Relative(IVec2::new(-7, 0))), node(Placement::Relative(IVec2::new(-7, 0)))],
		wires: vec![wire(0, 1, 0), wire(1, 0, 0)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(0)).owner, None);
	assert_eq!(layout.node(at(1)).owner, Some(at(0)));
}

#[test]
fn a_stacked_layer_is_drawn_below_the_content_of_the_layer_above() {
	let layout = Layout::new(&stack_with_tall_content(1));

	assert_eq!(layout.node(at(3)).position, IVec2::new(30 - CHAIN_WIDTH, 13));
	assert_eq!(layout.node(at(4)).position, IVec2::new(30, 13 + 4));
	assert_eq!(layout.node(at(4)).elongation, IVec2::new(0, 4));
	assert_eq!(layout.node(at(5)).position, layout.node(at(4)).position - IVec2::new(CHAIN_WIDTH, 0));
}

#[test]
fn a_stacked_layer_fits_beside_content_further_left() {
	let layout = Layout::new(&stack_with_tall_content(2));

	assert_eq!(layout.node(at(3)).position, IVec2::new(30 - 2 * CHAIN_WIDTH, 13));
	assert_eq!(layout.node(at(4)).elongation, IVec2::ZERO);
}

#[test]
fn the_connection_carrying_a_held_node_is_not_drawn_longer() {
	let mut graph = stack_with_tall_content(1);
	graph.nodes[5].held = true;

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(4)).elongation, IVec2::ZERO);
	assert_eq!(layout.node(at(5)).position, layout.node(at(4)).position - IVec2::new(CHAIN_WIDTH, 0));
}

#[test]
fn a_held_layer_stays_where_it_is_stored() {
	let mut graph = stack_with_tall_content(1);
	graph.nodes[4].held = true;

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(4)).position, IVec2::new(30, 13));
	assert_eq!(layout.node(at(4)).elongation, IVec2::ZERO);
}

#[test]
fn a_node_covering_its_reader_is_drawn_left_of_it() {
	let graph = Graph {
		nodes: vec![node(Placement::Absolute(IVec2::new(30, 10))), node(Placement::Relative(IVec2::new(-2, 1)))],
		wires: vec![wire(1, 0, 1)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(1)).position, IVec2::new(30 - 5 - COLUMNS_BETWEEN_NODES, 11));
}

#[test]
fn a_waypoint_takes_no_room_and_stays_where_it_is_put() {
	let graph = Graph {
		nodes: vec![node(Placement::Absolute(IVec2::new(30, 10))), waypoint(Placement::Relative(IVec2::new(-1, 1))), node(CHAINED)],
		wires: vec![wire(1, 0, 0), wire(2, 1, 0)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(1)).position, IVec2::new(29, 11));
	assert_eq!(layout.node(at(2)).position, IVec2::new(29 - CHAIN_WIDTH, 11));
}

#[test]
fn a_shared_node_belongs_to_its_highest_reader() {
	let graph = Graph {
		nodes: vec![node(Placement::Absolute(IVec2::new(30, 20))), node(Placement::Absolute(IVec2::new(30, 10))), node(CHAINED)],
		wires: vec![wire(2, 0, 0), wire(2, 1, 0)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(2)).owner, Some(at(1)));
	assert_eq!(layout.node(at(2)).position, IVec2::new(30 - CHAIN_WIDTH, 10));
}

#[test]
fn a_shared_node_is_placed_from_the_left_of_its_readers_level_with_the_highest() {
	let graph = Graph {
		nodes: vec![
			node(Placement::Absolute(IVec2::new(20, 30))),
			node(Placement::Absolute(IVec2::new(30, 10))),
			node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 2))),
		],
		wires: vec![wire(2, 0, 0), wire(2, 1, 0)],
	};

	let layout = Layout::new(&graph);

	assert_eq!(layout.node(at(2)).owner, Some(at(1)));
	assert_eq!(layout.node(at(2)).anchor, IVec2::new(20, 10));
	assert_eq!(layout.node(at(2)).position, IVec2::new(20 - CHAIN_WIDTH, 12));
}

#[test]
fn a_shared_node_stored_beside_another_reader_takes_no_room_from_its_owner() {
	let only_read_by_the_chain = Layout::new(&far_upstream_beside_tall_content(false));
	assert_eq!(only_read_by_the_chain.node(at(5)).position, IVec2::new(13, 40));
	assert_ne!(only_read_by_the_chain.node(at(1)).elongation, IVec2::ZERO);

	let also_read_from_beside = Layout::new(&far_upstream_beside_tall_content(true));
	assert_eq!(also_read_from_beside.node(at(5)).owner, Some(at(3)));
	assert_eq!(also_read_from_beside.node(at(5)).position, IVec2::new(13, 40));
	assert_eq!(also_read_from_beside.node(at(1)).elongation, IVec2::ZERO);
}

#[test]
fn a_fitted_layout_stored_as_offsets_needs_no_further_fitting() {
	let graph = stack_with_tall_content(1);
	let first = Layout::new(&graph);

	let mut refitted = graph.clone();
	for (index, node) in (0..).zip(&mut refitted.nodes) {
		if first.node(at(index)).owner.is_some() {
			node.placement = Placement::Relative(first.node(at(index)).position - first.node(at(index)).anchor);
		}
	}
	let second = Layout::new(&refitted);

	for index in 0..6 {
		assert_eq!(second.node(at(index)).position, first.node(at(index)).position);
		assert_eq!(second.node(at(index)).elongation, IVec2::ZERO);
	}
}

#[test]
fn a_layers_reach_covers_its_chain() {
	let graph = stack_with_tall_content(1);
	let layout = Layout::new(&graph);
	let reach = layout.reach(&graph, at(0));

	assert!(reach.contains(layout.node(at(2)).position));
	assert!(!reach.contains(layout.node(at(3)).position));
}

#[test]
fn an_empty_rect_intersects_nothing() {
	let cell = graph_layout::Rect::new(IVec2::ZERO, IVec2::ONE);
	let empty = graph_layout::Rect::new(IVec2::ZERO, IVec2::new(0, 1));

	assert!(cell.intersects(&cell));
	assert!(!cell.intersects(&empty));
	assert!(!empty.intersects(&cell));
}
