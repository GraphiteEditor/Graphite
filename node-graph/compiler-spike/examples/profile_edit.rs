use compiler_spike::lean::Lean;
use compiler_spike::lean::document::{Implementation, Input};
use compiler_spike::synthetic::rectangle_layers;

fn main() {
	let mut lean = Lean::load();
	let mut document = lean.convert_document(&rectangle_layers(1600));
	lean.compile_incremental(&mut document).unwrap();

	let transform = lean.strings.get("transform_nodes::transform_nodes::TransformNode").unwrap();
	let root = &document.definitions[document.root.index()];
	let mut bottom = None;
	'search: for (index, definition) in document.definitions.iter().enumerate() {
		for (local, node) in definition.nodes.iter().enumerate() {
			if matches!(node.implementation, Implementation::Proto(identifier) if identifier.0 == transform)
				&& let Some(input) = definition.node_inputs(node).iter().position(|input| matches!(input, Input::Value(_)))
			{
				bottom = Some((index as u32, local as u32, input));
				break 'search;
			}
		}
	}
	let _ = root;
	let (definition, local, input) = bottom.expect("a transform value should exist");

	let mut timings = Vec::new();
	for edit in 0..300_u64 {
		document.set_value(definition, local, input, 0xED17 + edit);
		let start = std::time::Instant::now();
		std::hint::black_box(lean.compile_incremental(&mut document).unwrap().output);
		timings.push((start.elapsed(), lean.ir.len()));
	}
	for range in [0..10, 100..110, 290..300] {
		let average = timings[range.clone()].iter().map(|(time, _)| *time).sum::<std::time::Duration>() / 10;
		println!("edits {range:?}: {average:.3?} per edit, store {} nodes", timings[range.end - 1].1);
	}
}
