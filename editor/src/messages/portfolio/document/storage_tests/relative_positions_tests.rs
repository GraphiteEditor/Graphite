//! Opening a document converts its absolute and stacked positions to positions relative to the nodes reading them, without moving any node.

use super::test_support::{node_paths, open_demo};
use crate::messages::portfolio::document::utility_types::network_interface::{LayerPersistentMetadata, LayerPosition, NodeLocator, NodeNetworkInterface, NodeTypePersistentMetadata};
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

/// Rewrites the converted positions the way older documents stored them, stacked layers as `Stack` and the rest as `Absolute`.
fn revert_to_stored_positions(interface: &mut NodeNetworkInterface, positions: &DrawnPositions) {
	for ((network_path, node_id), position) in positions {
		if !interface.is_relative(node_id, network_path) {
			continue;
		}

		let stack_offset = interface.stack_offset(node_id, network_path);
		let Some(mut node) = interface.node_mut(NodeLocator::new(*node_id, network_path)) else { continue };
		match (stack_offset, position) {
			(Some(stack_offset), _) => {
				node.set_node_type(NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
					position: LayerPosition::Stack(stack_offset as u32),
				}));
			}
			(None, Some(position)) => {
				node.set_absolute_position(*position);
			}
			(None, None) => {}
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
			let read_by_a_node = interface
				.with_outward_wires(network_path, |outward_wires| {
					outward_wires
						.iter()
						.any(|(output, readers)| output.node_id() == Some(*node_id) && readers.iter().any(|reader| reader.node_id().is_some()))
				})
				.unwrap_or_default();
			assert!(
				!read_by_a_node || !interface.is_absolute(node_id, network_path),
				"{demo}: node {node_id} is read by another node, so it should be relative"
			);
		}

		revert_to_stored_positions(interface, &converted);
		interface.convert_positions_to_relative();

		let reconverted = editable_positions(interface);
		let moved: Vec<_> = converted.iter().zip(&reconverted).filter(|(before, after)| before != after).collect();
		assert!(moved.is_empty(), "{demo}: converting should leave every node where it was drawn, but moved {moved:?}");
	}
}
