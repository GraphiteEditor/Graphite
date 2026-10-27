//! Times incremental compiles after typical edits against cold compiles. Run with `cargo run --release --bin incremental`.

use compiler_spike::lean::document::{Implementation, Input};
use compiler_spike::lean::{Document, Lean};
use compiler_spike::synthetic::rectangle_layers;
use compiler_spike::workload::{Network, load_network};
use std::time::{Duration, Instant};

const TRANSFORM: &str = "transform_nodes::transform_nodes::TransformNode";
const RUNS: usize = 30;

fn transform_values(lean: &Lean, document: &Document) -> Vec<(u32, u32, usize)> {
	let mut values = Vec::new();
	for (definition_index, definition) in document.definitions.iter().enumerate() {
		for (local, node) in definition.nodes.iter().enumerate() {
			let Implementation::Proto(identifier) = node.implementation else { continue };
			if lean.identifier_name(identifier) != TRANSFORM {
				continue;
			}
			for (input, value) in definition.node_inputs(node).iter().enumerate() {
				if matches!(value, Input::Value(_)) {
					values.push((definition_index as u32, local as u32, input));
				}
			}
		}
	}
	values
}

fn median(mut samples: Vec<Duration>) -> Duration {
	samples.sort();
	samples[samples.len() / 2]
}

fn report(name: &str, network: &Network) {
	let mut lean = Lean::load();
	let mut document = lean.convert_document(network);

	let cold = median((0..RUNS).map(|_| time(|| lean.compile(&document).map(|compiled| compiled.output))).collect());
	let warmup = time(|| lean.compile_incremental(&mut document).map(|compiled| compiled.output));
	let unchanged = median((0..RUNS).map(|_| time(|| lean.compile_incremental(&mut document).map(|compiled| compiled.output))).collect());
	let unchanged_statistics = lean.instance_statistics();

	println!(
		"\n{name}: cold {cold:.3?}, first incremental {warmup:.3?}, unchanged recompile {unchanged:.3?} ({} instances reused)",
		unchanged_statistics.instance_hits
	);
	println!(
		"{:>14} {:>12} {:>9} {:>9} {:>12} {:>9}",
		"edited value", "incremental", "reused", "rebuilt", "store nodes", "recomputed"
	);

	let values = transform_values(&lean, &document);
	if values.is_empty() {
		return;
	}
	let mut next_value = 0xED17_u64;
	for (label, index) in [("bottom layer", 0), ("middle layer", values.len() / 2), ("top layer", values.len() - 1)] {
		let (definition, local, input) = values[index];
		let samples: Vec<_> = (0..RUNS)
			.map(|_| {
				next_value += 1;
				document.set_value(definition, local, input, next_value);
				time(|| lean.compile_incremental(&mut document).map(|compiled| compiled.output))
			})
			.collect();
		let statistics = lean.instance_statistics();
		println!(
			"{label:>14} {:>12.3?} {:>9} {:>9} {:>12} {:>9}",
			median(samples),
			statistics.instance_hits,
			statistics.instance_misses,
			lean.ir.len(),
			statistics.nodes_invalidated
		);
	}
}

fn time<T>(compile: impl FnOnce() -> Result<T, String>) -> Duration {
	let start = Instant::now();
	std::hint::black_box(compile().expect("the workload should compile"));
	start.elapsed()
}

fn main() {
	report("red-dress", &load_network("red-dress"));
	for layers in [200, 1600] {
		report(&format!("rectangles({layers})"), &rectangle_layers(layers));
	}
}
