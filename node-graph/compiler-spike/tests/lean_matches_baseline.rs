use compiler_spike::baseline::Baseline;
use compiler_spike::baseline::proto::ConstructionArgs;
use compiler_spike::lean::Lean;
use compiler_spike::lean::executor::Executor;
use compiler_spike::workload::{DEMO_ART, load_expected, load_network};
use std::collections::{BTreeMap, HashSet};

/// The lean pass hash-conses as it builds, so its IR should equal the memoized baseline's graph after stable ID deduplication.
#[test]
fn lean_matches_the_memoized_baseline() {
	let mut baseline = Baseline::load();
	baseline.memoize_context_analysis = true;
	let mut lean = Lean::load();
	let expected = load_expected();

	for name in DEMO_ART.into_iter().chain(["rectangle-layers"]) {
		let network = baseline.load_network(name);
		let proto_network = baseline.compile(baseline.preprocess(network)).unwrap();
		let unique: HashSet<_> = proto_network.nodes.iter().map(|(id, node)| (*id, node.identifier.to_string())).collect();
		let mut baseline_counts: BTreeMap<String, usize> = BTreeMap::new();
		for (_, identifier) in unique {
			*baseline_counts.entry(identifier).or_default() += 1;
		}

		// The baseline applies promotions while constructing the executor, where lean has an adapter node per promoted input
		let executor = baseline.build_executor(proto_network.clone()).unwrap_or_else(|error| panic!("{name} failed to type: {error}"));
		let mut adapters = HashSet::new();
		for (id, node) in &proto_network.nodes {
			let (Some(promotions), ConstructionArgs::Nodes(inputs)) = (executor.typing_context.promotions(*id), &node.construction_args) else {
				continue;
			};
			for (index, promotion) in promotions {
				adapters.insert((inputs[*index], promotion.adapter_identifier()));
			}
		}
		for (_, identifier) in adapters {
			*baseline_counts.entry(identifier).or_default() += 1;
		}

		let document = lean.convert_document(&load_network(name));
		let compiled = lean.compile(&document).unwrap_or_else(|error| panic!("{name} failed to compile: {error}"));
		let (output, output_type) = (compiled.output, compiled.typing.info[compiled.output as usize].return_type);
		let executor_nodes = Executor::new(&compiled).nodes.len();
		let identifiers: Vec<_> = compiled.ir.nodes.iter().map(|node| node.identifier).collect();
		assert_eq!(executor_nodes, identifiers.len());

		let mut lean_counts: BTreeMap<String, usize> = BTreeMap::new();
		for identifier in identifiers {
			*lean_counts.entry(lean.identifier_name(identifier).to_string()).or_default() += 1;
		}

		let differing: Vec<_> = baseline_counts
			.keys()
			.chain(lean_counts.keys())
			.collect::<std::collections::BTreeSet<_>>()
			.into_iter()
			.filter(|identifier| baseline_counts.get(*identifier) != lean_counts.get(*identifier))
			.map(|identifier| {
				(
					identifier.clone(),
					baseline_counts.get(identifier).copied().unwrap_or(0),
					lean_counts.get(identifier).copied().unwrap_or(0),
				)
			})
			.collect();
		assert!(differing.is_empty(), "{name}: unique nodes per identifier differ (identifier, baseline, lean): {differing:#?}");

		let output_type = lean.typing.types.to_workload(output_type);
		assert_eq!(Some(output_type), expected[name].output_type, "{name}: output type");
		assert!(output < lean.ir.len() as u32);
	}
}
