//! Converting a document's positions, as importing an older document does, makes the absolute positions of nodes other nodes read relative to them.

use super::test_support::{node_paths, open_demo};
use crate::messages::portfolio::document::utility_types::network_interface::{NodeLocator, NodeNetworkInterface};
use glam::IVec2;
use graphene_std::uuid::NodeId;

const DEMOS: [&str; 8] = [
	"changing-seasons.graphite",
	"isometric-fountain.graphite",
	"marbled-mandelbrot.graphite",
	"painted-dreams.graphite",
	"parametric-dunescape.graphite",
	"procedural-string-lights.graphite",
	"red-dress.graphite",
	"valley-of-spires.graphite",
];

/// Each node in an editable network, by its network path and ID, with where it is drawn.
type DrawnPositions = Vec<((Vec<NodeId>, NodeId), Option<IVec2>)>;

fn editable_positions(interface: &NodeNetworkInterface) -> DrawnPositions {
	node_paths(interface)
		.into_iter()
		.filter(|(network_path, _)| !interface.is_read_only(network_path))
		.map(|(network_path, node_id)| {
			let position = interface.position(&node_id, &network_path);
			((network_path, node_id), position)
		})
		.collect()
}

/// Stores every node where it is drawn as an absolute position, as an older document would.
fn store_drawn_positions_as_absolute(interface: &mut NodeNetworkInterface, positions: &DrawnPositions) {
	for ((network_path, node_id), position) in positions {
		let Some(position) = position else { continue };
		if let Some(mut node) = interface.node_mut(NodeLocator::new(*node_id, network_path)) {
			node.set_absolute_position(*position);
		}
		interface.invalidate_positions(network_path);
	}
}

#[test]
fn converting_a_demo_makes_every_read_node_relative_where_it_was_drawn() {
	for demo in DEMOS {
		let mut document = open_demo(demo);
		let interface = &mut document.network_interface;

		interface.convert_positions_to_relative();
		let converted = editable_positions(interface);
		assert!(converted.iter().all(|(_, position)| position.is_some()), "{demo}: every node should resolve a position");

		for ((network_path, node_id), _) in &converted {
			let node_readers = interface
				.with_outward_wires(network_path, |outward_wires| {
					outward_wires
						.iter()
						.filter(|(output, _)| output.node_id() == Some(*node_id))
						.map(|(_, readers)| readers.iter().filter(|reader| reader.node_id().is_some()).count())
						.sum::<usize>()
				})
				.unwrap_or_default();
			assert!(
				node_readers == 0 || !interface.is_absolute(node_id, network_path),
				"{demo}: node {node_id} is read by another node, so it should be relative"
			);
		}

		store_drawn_positions_as_absolute(interface, &converted);
		interface.convert_positions_to_relative();

		let reconverted = editable_positions(interface);
		let moved: Vec<_> = converted.iter().zip(&reconverted).filter(|(before, after)| before != after).collect();
		assert!(moved.is_empty(), "{demo}: a layout stored where it is drawn should need no further fitting, but moved {moved:?}");
	}
}

#[test]
#[ignore]
fn count_the_nodes_fitting_moves_in_each_demo() {
	for demo in DEMOS {
		let document = open_demo(demo);
		let interface = &document.network_interface;
		let node_ids: Vec<NodeId> = interface.document_network().nodes.keys().copied().collect();
		let elongated = interface
			.with_layout(&[], |layout| node_ids.iter().filter(|node_id| layout.elongation(node_id) != IVec2::ZERO).count())
			.unwrap_or_default();
		let moved = interface
			.with_layout(&[], |layout| node_ids.iter().filter(|node_id| layout.drawn_away_from_stored(node_id) != IVec2::ZERO).count())
			.unwrap_or_default();
		println!(
			"{demo:34} {:4} nodes | {elongated:3} connections elongated | {moved:3} nodes drawn away from their stored place",
			node_ids.len()
		);
		{
			for node_id in &node_ids {
				let Some((elongation, owner)) = interface.with_layout(&[], |layout| (layout.elongation(node_id), layout.owner(node_id))) else {
					continue;
				};
				if elongation == IVec2::ZERO {
					continue;
				}
				let owner_name = owner.map(|owner| interface.display_name(&owner, &[])).unwrap_or_default();
				println!(
					"    {:28} layer {:5} by {elongation:>12} from owner {owner_name:28} | stored {:?} drawn {:?}",
					interface.display_name(node_id, &[]),
					interface.is_layer(node_id, &[]),
					interface.node_metadata(node_id, &[]).map(|metadata| metadata.persistent_metadata.node_type_metadata.clone()),
					interface.position(node_id, &[]),
				);
			}
		}
	}
}

#[test]
fn a_dragged_layer_moves_one_row_per_step_through_content_it_would_otherwise_clear() {
	let mut document = open_demo("changing-seasons.graphite");
	let interface = &mut document.network_interface;
	let node_ids: Vec<NodeId> = interface.document_network().nodes.keys().copied().collect();
	let Some(solid_backdrop) = node_ids.into_iter().find(|node_id| interface.display_name(node_id, &[]) == "Solid Backdrop") else {
		panic!("changing-seasons has a Solid Backdrop layer");
	};
	let start = interface.position(&solid_backdrop, &[]).expect("the layer is drawn");

	interface.selected_nodes_mut(&[]).expect("the root network has a selection").set_selected_nodes(vec![solid_backdrop]);
	interface.drag_selected_nodes(IVec2::ZERO, false, &[]);
	assert_eq!(interface.position(&solid_backdrop, &[]), Some(start), "Starting to drag the layer should not move it");

	let rows = (1..=10).chain((0..10).rev());
	for row in rows {
		interface.drag_selected_nodes(IVec2::new(0, row), false, &[]);
		assert_eq!(
			interface.position(&solid_backdrop, &[]),
			Some(start + IVec2::new(0, row)),
			"A dragged layer should move one row per step"
		);
	}

	interface.finish_drag(&[]);
	assert_eq!(interface.position(&solid_backdrop, &[]), Some(start), "Released where it started, the layer should stay there");
}

#[test]
fn pushing_a_layer_up_against_the_layer_above_carries_it_up() {
	let mut document = open_demo("procedural-string-lights.graphite");
	let interface = &mut document.network_interface;
	let node_ids: Vec<NodeId> = interface.document_network().nodes.keys().copied().collect();
	let find = |interface: &NodeNetworkInterface, name: &str| node_ids.iter().copied().find(|node_id| interface.display_name(node_id, &[]) == name);
	let (Some(bulb_shape), Some(bulb_glow)) = (find(interface, "Bulb Shape"), find(interface, "Bulb Glow Gradient")) else {
		panic!("procedural-string-lights has a Bulb Shape layer stacked under a Bulb Glow Gradient layer");
	};
	assert!(interface.is_stack(&bulb_shape, &[]), "fixture is wrong");
	let positions = |interface: &NodeNetworkInterface| [bulb_shape, bulb_glow].map(|layer| interface.position(&layer, &[]).expect("every layer is drawn"));
	let start = positions(interface);

	interface.selected_nodes_mut(&[]).expect("the root network has a selection").set_selected_nodes(vec![bulb_shape]);
	for rows in 1..=4 {
		interface.drag_selected_nodes(IVec2::new(0, -rows), false, &[]);
		assert_eq!(positions(interface), start.map(|position| position - IVec2::new(0, rows)), "The layer above should be carried up");
	}
}

#[test]
fn the_layout_kept_after_a_drag_matches_the_network_as_stored() {
	let mut document = open_demo("procedural-string-lights.graphite");
	let interface = &mut document.network_interface;
	let node_ids: Vec<NodeId> = interface.document_network().nodes.keys().copied().collect();

	for (node_id, offset) in node_ids.iter().flat_map(|node_id| [IVec2::new(3, -5), IVec2::new(-4, 6)].map(|offset| (*node_id, offset))) {
		interface.selected_nodes_mut(&[]).expect("the root network has a selection").set_selected_nodes(vec![node_id]);
		interface.drag_selected_nodes(offset, false, &[]);
		interface.finish_drag(&[]);

		let kept = interface.positions(node_ids.iter().copied(), &[]);
		let Some((_, indices, graph)) = interface.layout_graph(&[]) else {
			panic!("the root network is laid out")
		};
		let fresh = graph_layout::Layout::new(&graph);
		for (other, position) in kept {
			assert_eq!(
				Some(position),
				indices.get(&other).map(|index| fresh.node(*index).position),
				"After dragging {node_id} by {offset}, node {other} should be where the stored network places it"
			);
		}
	}
}
