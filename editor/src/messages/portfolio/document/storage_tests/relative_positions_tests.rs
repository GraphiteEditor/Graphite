//! Opening a document converts the absolute, chained, and stacked positions of nodes other nodes read to positions relative to them.

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
fn opening_a_demo_makes_every_read_node_relative_where_it_was_drawn() {
	for demo in DEMOS {
		let mut document = open_demo(demo);
		let interface = &mut document.network_interface;

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
