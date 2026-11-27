mod common;

use common::{CHAINED, at, layer, node, stacked, wire};
use glam::IVec2;
use graph_layout::{Drag, Graph, Layout, Placement};

fn closed_up_stack() -> Graph {
	Graph {
		nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), layer(stacked(0)), layer(stacked(0))],
		wires: vec![wire(1, 0, 0), wire(2, 1, 0)],
	}
}

fn positions_after_dragging_the_bottom_layer(offset: IVec2) -> [IVec2; 3] {
	let start = closed_up_stack();
	let (_, layout) = Drag::new(start.clone(), Layout::new(&start), vec![at(2)], false).moved(offset);
	[0, 1, 2].map(|index| layout.node(at(index)).position)
}

fn positions_before() -> [IVec2; 3] {
	let layout = Layout::new(&closed_up_stack());
	[0, 1, 2].map(|index| layout.node(at(index)).position)
}

#[test]
fn a_drag_back_to_where_it_started_leaves_everything_where_it_was_drawn() {
	assert_eq!(positions_after_dragging_the_bottom_layer(IVec2::ZERO), positions_before());
}

#[test]
fn a_layer_dragged_up_against_the_layer_above_carries_it_up() {
	let [_, above, dragged] = positions_before();
	let [_, moved_above, moved] = positions_after_dragging_the_bottom_layer(IVec2::new(0, -2));

	assert_eq!(moved, dragged + IVec2::new(0, -2));
	assert_eq!(moved_above, above + IVec2::new(0, -2));
}

#[test]
fn a_drag_below_its_start_leaves_the_layers_above_in_place() {
	let [top, above, dragged] = positions_before();
	let [moved_top, moved_above, moved] = positions_after_dragging_the_bottom_layer(IVec2::new(0, 1));

	assert_eq!(moved, dragged + IVec2::new(0, 1));
	assert_eq!(moved_above, above);
	assert_eq!(moved_top, top);
}

#[test]
fn the_layers_a_drag_carries_up_return_as_it_comes_back_down() {
	let start = closed_up_stack();
	let mut drag = Drag::new(start.clone(), Layout::new(&start), vec![at(2)], false);

	let _ = drag.moved(IVec2::new(0, -4));
	let (returned, _) = drag.moved(IVec2::ZERO);

	assert_eq!(
		returned.nodes.iter().map(|node| node.placement).collect::<Vec<_>>(),
		start.nodes.iter().map(|node| node.placement).collect::<Vec<_>>()
	);
}

#[test]
fn a_layer_pushed_up_with_its_upstream_kept_carries_the_layer_above_from_the_first_row() {
	let start = Graph {
		nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), layer(stacked(3)), layer(stacked(0))],
		wires: vec![wire(1, 0, 0), wire(2, 1, 0)],
	};
	let before = Layout::new(&start);

	let (_, layout) = Drag::new(start.clone(), Layout::new(&start), vec![at(2)], true).moved(IVec2::new(0, -2));

	assert_eq!(layout.node(at(2)).position, before.node(at(2)).position + IVec2::new(0, -2));
	assert_eq!(layout.node(at(1)).position, before.node(at(1)).position + IVec2::new(0, -2));
}

#[test]
fn a_layer_dragged_up_with_its_upstream_kept_carries_its_chain_and_leaves_its_stack() {
	let start = Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			node(CHAINED),
			node(Placement::Relative(IVec2::new(-8, 0))),
			layer(stacked(0)),
		],
		wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, 0, 0)],
	};
	let before = Layout::new(&start);
	let offset = IVec2::new(0, -4);

	let (_, layout) = Drag::new(start.clone(), Layout::new(&start), vec![at(0)], true).moved(offset);

	for carried in [0, 1, 2] {
		assert_eq!(layout.node(at(carried)).position, before.node(at(carried)).position + offset);
	}
	assert_eq!(layout.node(at(3)).position, before.node(at(3)).position);
}

fn stack_of_layers_with_chains() -> Graph {
	let layers = [layer(Placement::Absolute(IVec2::new(30, 10))), layer(stacked(0)), layer(stacked(0))];
	let chains = [node(CHAINED), node(CHAINED), node(CHAINED)];
	let stack_wires = [wire(1, 0, 0), wire(2, 1, 0)];
	let chain_wires = [wire(3, 0, 1), wire(4, 1, 1), wire(5, 2, 1)];
	Graph {
		nodes: layers.into_iter().chain(chains).collect(),
		wires: stack_wires.into_iter().chain(chain_wires).collect(),
	}
}

fn moved_by_dragging(graph: &Graph, dragged: &[u32], offset: IVec2) -> Vec<u32> {
	let before = Layout::new(graph);
	let (_, layout) = Drag::new(graph.clone(), Layout::new(graph), dragged.iter().copied().map(at).collect(), true).moved(offset);
	let node_count = u32::try_from(graph.nodes.len()).unwrap();
	(0..node_count).filter(|index| layout.node(at(*index)).position != before.node(at(*index)).position).collect()
}

#[test]
fn a_layer_in_the_middle_of_a_stack_dragged_sideways_with_its_upstream_kept_moves_its_whole_stack() {
	assert_eq!(moved_by_dragging(&stack_of_layers_with_chains(), &[1], IVec2::new(10, 0)), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn a_chain_node_dragged_with_its_upstream_kept_moves_alone() {
	assert_eq!(moved_by_dragging(&stack_of_layers_with_chains(), &[4], IVec2::new(0, 10)), vec![4]);
}

#[test]
fn a_layer_pushed_up_with_its_upstream_kept_carries_the_closed_up_layers_above_with_their_chains() {
	let graph = stack_of_layers_with_chains();
	let mut stack_with_one_more_below = graph.clone();
	stack_with_one_more_below.nodes.push(layer(stacked(0)));
	stack_with_one_more_below.wires.push(wire(6, 2, 0));

	assert_eq!(moved_by_dragging(&stack_with_one_more_below, &[2], IVec2::new(0, -1)), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn layers_of_one_stack_dragged_down_together_with_their_upstream_kept_carry_their_chains() {
	assert_eq!(moved_by_dragging(&stack_of_layers_with_chains(), &[1, 2], IVec2::new(0, 4)), vec![1, 2, 4, 5]);
}

fn artboard_holding_a_stack_of_layers_with_chains() -> Graph {
	let artboard = layer(Placement::Absolute(IVec2::new(30, 10)));
	let layers = [layer(Placement::Relative(IVec2::new(-8, 3))), layer(stacked(0)), layer(stacked(0)), layer(stacked(0))];
	let chains = (0..4).flat_map(|_| [node(CHAINED), node(CHAINED), node(CHAINED)]);
	let stack_wires = [wire(1, 0, 1), wire(2, 1, 0), wire(3, 2, 0), wire(4, 3, 0)];
	let chain_wires = (0..4).flat_map(|layer| {
		let first = 5 + layer * 3;
		[wire(first, layer + 1, 1), wire(first + 1, first, 0), wire(first + 2, first + 1, 0)]
	});
	Graph {
		nodes: [artboard].into_iter().chain(layers).chain(chains).collect(),
		wires: stack_wires.into_iter().chain(chain_wires).collect(),
	}
}

#[test]
fn releasing_a_drag_draws_every_node_where_the_drag_drew_it() {
	let stack = artboard_holding_a_stack_of_layers_with_chains();
	let reading_a_tree = artboard_holding_a_layer_whose_chain_reads_a_tree_of_its_own();
	let drags = [
		(&stack, vec![1, 2], IVec2::new(5, 0), true),
		(&stack, vec![1, 2], IVec2::new(5, 0), false),
		(&stack, vec![2], IVec2::new(5, 0), false),
		(&stack, vec![3], IVec2::new(0, -2), true),
		(&stack, vec![3], IVec2::new(0, 3), true),
		(&stack, vec![3], IVec2::new(0, -2), false),
		(&stack, vec![3], IVec2::new(0, 3), false),
		(&reading_a_tree, vec![2], IVec2::new(0, 3), true),
		(&reading_a_tree, vec![2], IVec2::new(5, 0), true),
	];
	for (start, dragged, offset, keep_upstream) in drags {
		let mut drag = Drag::new(start.clone(), Layout::new(start), dragged.iter().copied().map(at).collect(), keep_upstream);
		let (_, dragging) = drag.moved(offset);
		let released = Layout::new(&drag.released(offset));

		let node_count = u32::try_from(start.nodes.len()).unwrap();
		let drawn = |layout: &Layout| (0..node_count).map(|index| layout.node(at(index)).position).collect::<Vec<_>>();
		assert_eq!(drawn(&released), drawn(&dragging), "dragging {dragged:?} by {offset} keeping upstream: {keep_upstream}");
	}
}

#[test]
fn layers_closed_up_under_the_layer_holding_them_cannot_be_pushed_up_into_it() {
	let start = artboard_holding_a_stack_of_layers_with_chains();
	for keep_upstream in [false, true] {
		let (_, layout) = Drag::new(start.clone(), Layout::new(&start), vec![at(3)], keep_upstream).moved(IVec2::new(0, -2));
		assert_eq!(layout.node(at(1)).position, Layout::new(&start).node(at(1)).position, "keeping upstream: {keep_upstream}");
	}
}

fn artboard_holding_a_layer_whose_chain_reads_a_tree_of_its_own() -> Graph {
	Graph {
		nodes: vec![
			layer(Placement::Absolute(IVec2::new(30, 10))),
			layer(Placement::Relative(IVec2::new(-8, 3))),
			layer(stacked(0)),
			node(CHAINED),
			node(Placement::Absolute(IVec2::new(0, 30))),
			node(CHAINED),
		],
		wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, 2, 1), wire(4, 3, 0), wire(5, 4, 0)],
	}
}

#[test]
fn a_layer_dragged_with_its_upstream_kept_carries_the_trees_its_chain_reads() {
	let graph = artboard_holding_a_layer_whose_chain_reads_a_tree_of_its_own();

	assert_eq!(moved_by_dragging(&graph, &[2], IVec2::new(0, 3)), vec![2, 3, 4, 5]);
	assert_eq!(moved_by_dragging(&graph, &[2], IVec2::new(5, 0)), vec![1, 2, 3, 4, 5]);
}

#[test]
fn the_trees_a_blocked_layer_reads_stay_with_it() {
	assert_eq!(
		moved_by_dragging(&artboard_holding_a_layer_whose_chain_reads_a_tree_of_its_own(), &[2], IVec2::new(0, -2)),
		Vec::<u32>::new()
	);
}
