mod common;

use common::{at, layer, wire};
use glam::IVec2;
use graph_layout::{Drag, Graph, Layout, Placement};

fn closed_up_stack() -> Graph {
	Graph {
		nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), layer(Placement::Stack(0)), layer(Placement::Stack(0))],
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
		nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), layer(Placement::Stack(3)), layer(Placement::Stack(0))],
		wires: vec![wire(1, 0, 0), wire(2, 1, 0)],
	};
	let before = Layout::new(&start);

	let (_, layout) = Drag::new(start.clone(), Layout::new(&start), vec![at(2)], true).moved(IVec2::new(0, -2));

	assert_eq!(layout.node(at(2)).position, before.node(at(2)).position + IVec2::new(0, -2));
	assert_eq!(layout.node(at(1)).position, before.node(at(1)).position + IVec2::new(0, -2));
}
