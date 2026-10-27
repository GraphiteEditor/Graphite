use compiler_spike::lean::document::{Implementation, Input};
use compiler_spike::lean::{Document, Lean};
use compiler_spike::synthetic::rectangle_layers;
use compiler_spike::workload::load_network;
use std::collections::BTreeMap;

const TRANSFORM: &str = "transform_nodes::transform_nodes::TransformNode";

/// Unique nodes per identifier among the nodes reachable from the output, plus the output type.
fn summary(lean: &Lean, output: u32) -> (BTreeMap<String, usize>, compiler_spike::workload::Type) {
	let mut counts = BTreeMap::new();
	for id in lean.ir.reachable(output) {
		let identifier = lean.identifier_name(lean.ir.nodes[id as usize].identifier).to_string();
		*counts.entry(identifier).or_default() += 1;
	}
	(counts, lean.typing.types.to_workload(lean.typing.info[output as usize].return_type))
}

/// Every value input of a Transform node, in document order, as (definition, node, input).
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

fn assert_incremental_matches_cold(incremental: &mut Lean, cold: &mut Lean, document: &mut Document, context: &str) {
	let output = incremental.compile_incremental(document).unwrap().output;
	let incremental_summary = summary(incremental, output);
	let output = cold.compile(document).unwrap().output;
	let cold_summary = summary(cold, output);
	assert_eq!(incremental_summary, cold_summary, "{context}: the incremental compile differs from a cold compile");
}

#[test]
fn incremental_compiles_match_cold_compiles_after_value_edits() {
	let mut incremental = Lean::load();
	let mut cold = Lean::load();

	let mut document = incremental.convert_document(&rectangle_layers(20));
	let _ = cold.convert_document(&rectangle_layers(20));
	assert_incremental_matches_cold(&mut incremental, &mut cold, &mut document, "initial compile");
	assert_incremental_matches_cold(&mut incremental, &mut cold, &mut document, "unchanged recompile");
	assert!(incremental.instance_statistics().instance_misses == 0, "An unchanged recompile should hit every instance");

	let values = transform_values(&incremental, &document);
	for (edit, &(definition, local, input)) in [values[0], values[values.len() / 2], values[values.len() - 1]].iter().enumerate() {
		document.set_value(definition, local, input, 0xED17 + edit as u64);
		assert_incremental_matches_cold(&mut incremental, &mut cold, &mut document, &format!("edit {edit}"));
	}
}

#[test]
fn incremental_compiles_match_cold_compiles_on_the_demos() {
	for name in ["red-dress", "changing-seasons"] {
		let mut incremental = Lean::load();
		let mut cold = Lean::load();
		let network = load_network(name);
		let mut document = incremental.convert_document(&network);
		let _ = cold.convert_document(&network);

		assert_incremental_matches_cold(&mut incremental, &mut cold, &mut document, name);
		if let Some(&(definition, local, input)) = transform_values(&incremental, &document).first() {
			document.set_value(definition, local, input, 0xED17);
			assert_incremental_matches_cold(&mut incremental, &mut cold, &mut document, &format!("{name} after an edit"));
		}
	}
}
