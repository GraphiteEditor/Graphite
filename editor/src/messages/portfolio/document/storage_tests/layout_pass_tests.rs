//! Spike: the one-pass layout from `graph-layout` against the editor's per-node position resolution, on the demo artworks.

use super::test_support::open_demo;
use crate::messages::portfolio::document::utility_types::network_interface::{LayerPosition, NodeNetworkInterface, NodePosition, NodeTypePersistentMetadata};
use glam::IVec2;
use graph_craft::document::NodeInput;
use graph_layout::{Graph, Node, Placement, Wire, layout};
use graphene_std::uuid::NodeId;
use std::collections::HashMap;
use std::time::Instant;

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
const REPETITIONS: u32 = 200;

/// The root network of `interface` as the layout pass reads it, with the node each index stands for.
fn graph_of(interface: &NodeNetworkInterface) -> (Vec<NodeId>, Graph) {
	let network = interface.document_network();
	let node_ids: Vec<NodeId> = network.nodes.keys().copied().collect();
	let indices: HashMap<NodeId, u32> = node_ids.iter().enumerate().map(|(index, id)| (*id, index as u32)).collect();

	let nodes = node_ids
		.iter()
		.map(|id| {
			let metadata = interface.node_metadata(id, &[]).expect("every node has metadata");
			let (placement, is_layer) = match &metadata.persistent_metadata.node_type_metadata {
				NodeTypePersistentMetadata::Layer(layer) => match layer.position {
					LayerPosition::Absolute(position) => (Placement::Absolute(position), true),
					LayerPosition::Stack(gap) => (Placement::Stack(gap), true),
				},
				NodeTypePersistentMetadata::Node(node) => match *node.position() {
					NodePosition::Absolute(position) => (Placement::Absolute(position), false),
					NodePosition::Chain => (Placement::Chain, false),
				},
			};
			let height = interface.node_height(id, &[]);
			Node {
				placement,
				is_layer,
				rows: height.saturating_sub(1),
				width: 5,
				inputs: network.nodes[id].inputs.len() as u32,
				outputs: 1,
				chainable: !interface.is_redirect_dot(id, &[]) && !interface.hidden_primary_output(id, &[]),
			}
		})
		.collect();

	let wires = node_ids
		.iter()
		.enumerate()
		.flat_map(|(target, id)| {
			let indices = &indices;
			network.nodes[id].inputs.iter().enumerate().filter_map(move |(input, node_input)| match node_input {
				NodeInput::Node { node_id, output_index, .. } => Some(Wire {
					source: *indices.get(node_id)?,
					output: *output_index as u32,
					target: target as u32,
					input: input as u32,
				}),
				_ => None,
			})
		})
		.collect();

	(node_ids, Graph { nodes, wires })
}

#[test]
#[ignore]
fn layout_pass_matches_and_times_the_editor_resolver() {
	for demo in DEMOS {
		let mut document = open_demo(demo);
		let interface = &mut document.network_interface;
		let (node_ids, graph) = graph_of(interface);

		let pass = layout(&graph);
		let mismatches: Vec<(NodeId, Option<IVec2>, IVec2)> = node_ids
			.iter()
			.enumerate()
			.filter_map(|(index, id)| {
				let editor = interface.position(id, &[]);
				(editor != Some(pass.positions[index])).then_some((*id, editor, pass.positions[index]))
			})
			.collect();

		let chain_mismatches: Vec<(NodeId, Option<NodeId>, Option<NodeId>)> = node_ids
			.iter()
			.enumerate()
			.filter_map(|(index, id)| {
				let editor = interface.is_chain(id, &[]).then(|| interface.downstream_layer_for_chain_node(id, &[])).flatten();
				let derived = pass.chain_layers[index].map(|layer| node_ids[layer as usize]);
				(editor != derived).then_some((*id, editor, derived))
			})
			.collect();
		let chained = pass.chain_layers.iter().filter(|layer| layer.is_some()).count();

		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for id in &node_ids {
				std::hint::black_box(interface.position(id, &[]));
			}
		}
		let resolver = start.elapsed() / REPETITIONS;

		let start = Instant::now();
		for _ in 0..REPETITIONS {
			interface.unload_all_nodes_click_targets(&[]);
			for id in &node_ids {
				interface.load_node_click_targets(id, &[]);
			}
		}
		let click_targets = start.elapsed() / REPETITIONS;

		let start = Instant::now();
		for _ in 0..REPETITIONS {
			std::hint::black_box(graph_of(interface));
		}
		let conversion = start.elapsed() / REPETITIONS;

		let start = Instant::now();
		for _ in 0..REPETITIONS {
			std::hint::black_box(layout(std::hint::black_box(&graph)));
		}
		let one_pass = start.elapsed() / REPETITIONS;

		println!(
			"{demo:34} {:5} nodes {:5} wires | resolver {resolver:>10.1?} | click targets {click_targets:>10.1?} | pass {one_pass:>9.1?} (+ conversion {conversion:>9.1?}) | {} position mismatches | {chained} chained, {} chain mismatches",
			graph.nodes.len(),
			graph.wires.len(),
			mismatches.len(),
			chain_mismatches.len(),
		);
		for (id, editor, pass) in mismatches.iter().take(5) {
			println!("    position of {id:?}: editor {editor:?}, pass {pass:?}");
		}
		for (id, editor, derived) in chain_mismatches.iter().take(5) {
			println!("    chain layer of {id:?}: editor {editor:?}, derived {derived:?}");
		}
	}
}
