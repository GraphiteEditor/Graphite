use compiler_spike::baseline::proto::{ConstructionArgs, ProtoNetwork};
use compiler_spike::baseline::types::TypeNames;
use compiler_spike::baseline::{Baseline, CONTEXT_MODIFICATION, MEMOIZE};
use compiler_spike::workload::{DEMO_ART, load_expected};
use std::collections::{HashMap, HashSet};

#[test]
fn baseline_matches_graphite() {
	let mut baseline = Baseline::load();
	let expected = load_expected();

	for name in DEMO_ART.into_iter().chain(["rectangle-layers"]) {
		let network = baseline.load_network(name);
		let proto_network = baseline.compile(baseline.preprocess(network)).unwrap_or_else(|error| panic!("{name} failed to compile: {error}"));

		let unique_stable_ids: HashSet<_> = proto_network.nodes.iter().map(|(id, _)| *id).collect();
		let distinct_identifiers: HashSet<_> = proto_network.nodes.iter().map(|(_, node)| node.identifier.clone()).collect();
		let expected = &expected[name];
		let mut identifier_counts = std::collections::BTreeMap::<String, usize>::new();
		for (_, node) in &proto_network.nodes {
			*identifier_counts.entry(node.identifier.to_string()).or_default() += 1;
		}
		let differing: Vec<_> = identifier_counts
			.keys()
			.chain(expected.identifier_counts.keys())
			.collect::<std::collections::BTreeSet<_>>()
			.into_iter()
			.filter_map(|identifier| {
				let (actual, graphite) = (
					identifier_counts.get(identifier).copied().unwrap_or(0),
					expected.identifier_counts.get(identifier).copied().unwrap_or(0),
				);
				(actual != graphite).then(|| format!("{identifier}: {actual} instead of {graphite}"))
			})
			.collect();
		assert!(differing.is_empty(), "{name}: proto nodes per identifier differ:\n{}", differing.join("\n"));
		assert_eq!(proto_network.nodes.len(), expected.proto_nodes, "{name}: proto node count");
		assert_eq!(unique_stable_ids.len(), expected.unique_stable_ids, "{name}: unique stable ID count");
		assert_eq!(distinct_identifiers.len(), expected.distinct_identifiers, "{name}: distinct identifier count");

		let executor = baseline.build_executor(proto_network).unwrap_or_else(|error| panic!("{name} failed to type check: {error}"));
		let expected_output = expected.output_type.as_ref().map(TypeNames::convert_deserialized);
		assert_eq!(executor.output_type(), expected_output, "{name}: output type");
	}
}

/// Replicating the template's top layer back up to three layers rebuilds a graph equivalent to the template the editor drew.
#[test]
fn rectangle_layers_generator_matches_the_editor_template() {
	let mut baseline = Baseline::load();
	let expected = &load_expected()["rectangle-layers"];

	let network = baseline.convert_network(&compiler_spike::synthetic::rectangle_layers(3));
	let proto_network = baseline.compile(baseline.preprocess(network)).unwrap();
	let unique_stable_ids: HashSet<_> = proto_network.nodes.iter().map(|(id, _)| *id).collect();
	assert_eq!(proto_network.nodes.len(), expected.proto_nodes);
	assert_eq!(unique_stable_ids.len(), expected.unique_stable_ids);
	baseline.build_executor(proto_network).unwrap();
}

/// Unique nodes of the compiled network, and how many nullification nodes directly wrap another nullification node.
fn unique_nodes_and_nested_nullifications(proto_network: &ProtoNetwork) -> (usize, usize) {
	let nodes: HashMap<_, _> = proto_network.nodes.iter().map(|(id, node)| (*id, node)).collect();
	let first_input = |id| match &nodes[&id].construction_args {
		ConstructionArgs::Nodes(inputs) => inputs.first().copied(),
		_ => None,
	};
	let nullifications: HashSet<_> = proto_network.nodes.iter().filter(|(_, node)| node.identifier == CONTEXT_MODIFICATION).map(|(id, _)| *id).collect();
	let nested = nullifications
		.iter()
		.filter(|&&id| first_input(id).is_some_and(|memoize| nodes[&memoize].identifier == MEMOIZE && first_input(memoize).is_some_and(|inner| nullifications.contains(&inner))))
		.count();
	(nodes.len(), nested)
}

/// Graphite reanalyzed a shared node once per path reaching it before #4834, and each revisit wrapped its existing nullification in
/// another one stripping the same features. Memoizing the analysis removes exactly those redundant memoize and nullification pairs.
#[test]
fn memoized_context_analysis_only_removes_nested_nullifications() {
	let mut baseline = Baseline::load();

	for name in DEMO_ART {
		let network = baseline.load_network(name);
		let mut compile = |memoize| {
			baseline.memoize_context_analysis = memoize;
			unique_nodes_and_nested_nullifications(&baseline.compile(baseline.preprocess(network.clone())).unwrap())
		};
		let (faithful_nodes, faithful_nested) = compile(false);
		let (memoized_nodes, memoized_nested) = compile(true);

		assert_eq!(memoized_nested, 0, "{name}: memoized analysis left nested nullifications");
		assert_eq!(
			faithful_nodes - memoized_nodes,
			2 * faithful_nested,
			"{name}: memoizing removed more than the nested nullification pairs"
		);
	}
}
