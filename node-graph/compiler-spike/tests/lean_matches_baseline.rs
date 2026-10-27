use compiler_spike::baseline::proto::ConstructionArgs;
use compiler_spike::baseline::types::Type;
use compiler_spike::baseline::{Baseline, CONTEXT_MODIFICATION, MEMOIZE, VALUE_NODE};
use compiler_spike::lean::executor::Executor;
use compiler_spike::lean::{InputAdapters, Lean};
use compiler_spike::workload::{DEMO_ART, load_expected, load_network};
use std::collections::{BTreeMap, BTreeSet, HashSet};

type Counts = BTreeMap<String, usize>;

fn nested(ty: &Type) -> &Type {
	match ty {
		Type::Fn(_, output) => nested(output),
		_ => ty,
	}
}

/// Unique nodes per identifier after stable ID deduplication, with promotions counted as the adapter node per promoted
/// input that lean builds, and optionally without input adapters that leave their input's type unchanged.
fn baseline_counts(baseline: &mut Baseline, name: &str, skip_identity_adapters: bool) -> Counts {
	let network = baseline.load_network(name);
	let proto_network = baseline.compile(baseline.preprocess(network)).unwrap();
	let executor = baseline.build_executor(proto_network.clone()).unwrap_or_else(|error| panic!("{name} failed to type: {error}"));
	let typing = &executor.typing_context;

	let is_identity_adapter =
		|id, identifier: &str| identifier.starts_with("input_adapter<") && typing.type_of(id).is_some_and(|types| types.inputs.first().is_some_and(|input| nested(input) == &types.return_value));
	let unique: HashSet<_> = proto_network
		.nodes
		.iter()
		.filter(|(id, node)| !(skip_identity_adapters && is_identity_adapter(*id, &node.identifier)))
		.map(|(id, node)| (*id, node.identifier.to_string()))
		.collect();

	let mut adapters = HashSet::new();
	for (id, node) in &proto_network.nodes {
		let (Some(promotions), ConstructionArgs::Nodes(inputs)) = (typing.promotions(*id), &node.construction_args) else {
			continue;
		};
		for (index, promotion) in promotions {
			adapters.insert((inputs[*index], promotion.adapter_identifier()));
		}
	}

	let mut counts = Counts::new();
	for identifier in unique.into_iter().map(|(_, identifier)| identifier).chain(adapters.into_iter().map(|(_, identifier)| identifier)) {
		*counts.entry(identifier).or_default() += 1;
	}
	counts
}

fn lean_counts(lean: &mut Lean, name: &str) -> Counts {
	let expected = load_expected();
	let document = lean.convert_document(&load_network(name));
	let compiled = lean.compile(&document).unwrap_or_else(|error| panic!("{name} failed to compile: {error}"));
	let (output, output_type) = (compiled.output, compiled.typing.info[compiled.output as usize].return_type);
	assert_eq!(Executor::new(&compiled).nodes.len(), compiled.ir.len());

	let identifiers: Vec<_> = compiled.ir.nodes.iter().map(|node| node.identifier).collect();
	let mut counts = Counts::new();
	for identifier in identifiers {
		*counts.entry(lean.identifier_name(identifier).to_string()).or_default() += 1;
	}

	let output_type = lean.typing.types.to_workload(output_type);
	assert_eq!(Some(output_type), expected[name].output_type, "{name}: output type");
	assert!(output < lean.ir.len() as u32);
	counts
}

fn assert_counts_match(name: &str, baseline: &Counts, lean: &Counts, ignored: &[&str]) {
	let differing: Vec<_> = baseline
		.keys()
		.chain(lean.keys())
		.collect::<BTreeSet<_>>()
		.into_iter()
		.filter(|identifier| !ignored.contains(&identifier.as_str()) && baseline.get(*identifier) != lean.get(*identifier))
		.map(|identifier| (identifier.clone(), baseline.get(identifier).copied().unwrap_or(0), lean.get(identifier).copied().unwrap_or(0)))
		.collect();
	assert!(differing.is_empty(), "{name}: unique nodes per identifier differ (identifier, baseline, lean): {differing:#?}");
}

/// The lean pass hash-conses as it builds, so its IR should equal the memoized baseline's graph after stable ID deduplication.
#[test]
fn lean_matches_the_memoized_baseline() {
	let mut baseline = Baseline::load();
	baseline.memoize_context_analysis = true;
	let mut lean = Lean::load_with(InputAdapters::Preprocessed);

	for name in DEMO_ART.into_iter().chain(["rectangle-layers"]) {
		assert_counts_match(name, &baseline_counts(&mut baseline, name, false), &lean_counts(&mut lean, name), &[]);
	}
}

/// Without the preprocessor's wrappers, typing inserts exactly the input adapters that change a type. Values then feed
/// connectors directly instead of through a wrapper, which needs fewer nullifications, so those are not compared.
#[test]
fn resolved_adapters_match_the_converting_preprocessed_ones() {
	let mut baseline = Baseline::load();
	baseline.memoize_context_analysis = true;
	let mut lean = Lean::load_with(InputAdapters::Resolved);

	for name in DEMO_ART.into_iter().chain(["rectangle-layers"]) {
		let ignored = [MEMOIZE, CONTEXT_MODIFICATION, VALUE_NODE];
		assert_counts_match(name, &baseline_counts(&mut baseline, name, true), &lean_counts(&mut lean, name), &ignored);
	}
}
