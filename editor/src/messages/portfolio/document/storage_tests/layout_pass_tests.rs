//! Times laying out the demo artworks, building their click targets, and hit testing them, as the editor does on demand.

use super::test_support::open_demo;
use crate::messages::portfolio::document::utility_types::network_interface::{GlideOffsets, InputConnector, OutputConnector};
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use glam::DVec2;
use graphene_std::uuid::NodeId;
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

#[test]
fn wire_anchors_match_the_ports_of_every_demo_node() {
	for demo in DEMOS {
		let document = open_demo(demo);
		let interface = &document.network_interface;

		for node_id in interface.document_network().nodes.keys() {
			let ports = interface.compute_node_ports(node_id, &[]).expect("every node has ports");
			for index in 0..interface.number_of_inputs(node_id, &[]) {
				let input = InputConnector::node_at_index(*node_id, index);
				assert_eq!(interface.input_position(&input, &[]), ports.input_port_position(index), "{demo}: input {index} of node {node_id}");
			}
			for index in 0..interface.number_of_outputs(node_id, &[]) {
				let output = OutputConnector::node(*node_id, index);
				assert_eq!(interface.output_position(&output, &[]), ports.output_port_position(index), "{demo}: output {index} of node {node_id}");
			}
		}
	}
}

#[test]
#[ignore]
fn time_the_layout_click_targets_and_hit_tests() {
	for demo in DEMOS {
		let mut document = open_demo(demo);
		let interface = &mut document.network_interface;
		let node_ids: Vec<NodeId> = interface.document_network().nodes.keys().copied().collect();
		let Some(root) = node_ids.iter().copied().find(|node_id| interface.is_absolute(node_id, &[])) else {
			continue;
		};

		let start = Instant::now();
		for _ in 0..REPETITIONS {
			interface.shift_node(&root, glam::IVec2::X, &[]);
			std::hint::black_box(interface.position(&root, &[]));
		}
		let lay_out = start.elapsed() / REPETITIONS;

		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for node_id in &node_ids {
				std::hint::black_box(interface.node_click_targets(node_id, &[]));
			}
		}
		let all_click_targets = start.elapsed() / REPETITIONS;

		let inputs = interface.node_graph_input_connectors(&[]);
		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for input in &inputs {
				std::hint::black_box(interface.wire_update_from_input(input, GraphWireStyle::default(), &GlideOffsets::default(), &[]));
			}
		}
		let all_wires = start.elapsed() / REPETITIONS;

		let connected: Vec<_> = inputs.iter().filter_map(|input| Some((*input, interface.upstream_output_connector(input, &[])?))).collect();
		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for (input, output) in &connected {
				std::hint::black_box((interface.input_position(input, &[]), interface.output_position(output, &[])));
			}
		}
		let ends = start.elapsed() / REPETITIONS;
		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for (_, output) in &connected {
				let output_type = interface.output_type(output, &[]);
				std::hint::black_box((output_type.displayed_type(), output_type.is_list()));
			}
		}
		let types = start.elapsed() / REPETITIONS;
		let endpoints: Vec<_> = connected
			.iter()
			.filter_map(|(input, output)| Some((interface.output_position(output, &[])?, interface.input_position(input, &[])?)))
			.collect();
		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for (output, input) in &endpoints {
				std::hint::black_box(crate::messages::portfolio::document::utility_types::wires::build_vector_wire(
					*output,
					*input,
					false,
					false,
					GraphWireStyle::default(),
				));
			}
		}
		let paths = start.elapsed() / REPETITIONS;
		let built: Vec<_> = endpoints
			.iter()
			.map(|(output, input)| crate::messages::portfolio::document::utility_types::wires::build_vector_wire(*output, *input, false, false, GraphWireStyle::default()))
			.collect();
		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for path in &built {
				std::hint::black_box(path.to_svg());
			}
		}
		let svg = start.elapsed() / REPETITIONS;
		println!(
			"    {} wires | ends {ends:>8.1?} | types {types:>8.1?} | paths {paths:>8.1?} | svg strings {svg:>8.1?}",
			connected.len()
		);

		let node_graph_to_viewport = interface
			.network_metadata(&[])
			.expect("the root network exists")
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport;
		let clicks: Vec<DVec2> = node_ids
			.iter()
			.filter_map(|node_id| interface.node_bounding_box(node_id, &[]))
			.map(|[min, max]| node_graph_to_viewport.transform_point2((min + max) / 2.))
			.collect();
		let start = Instant::now();
		for _ in 0..REPETITIONS {
			for click in &clicks {
				std::hint::black_box(interface.node_from_click(*click, &[]));
			}
		}
		let hit_test = start.elapsed() / (REPETITIONS * clicks.len().max(1) as u32);
		let hits = clicks.iter().filter(|click| interface.node_from_click(**click, &[]).is_some()).count();

		println!(
			"{demo:34} {:4} nodes | lay out {lay_out:>8.1?} | all click targets {all_click_targets:>8.1?} | all wires {all_wires:>8.1?} | hit test {hit_test:>8.1?} per query ({hits} of {} centers hit)",
			node_ids.len(),
			clicks.len(),
		);
	}
}
