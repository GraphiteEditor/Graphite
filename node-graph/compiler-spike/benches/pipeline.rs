use compiler_spike::baseline::Baseline;
use compiler_spike::workload::DEMO_ART;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn baseline(c: &mut Criterion) {
	let mut baseline = Baseline::load();
	let demos: Vec<_> = DEMO_ART
		.iter()
		.map(|name| {
			let network = baseline.load_network(name);
			let preprocessed = baseline.preprocess(network.clone());
			let proto_network = baseline.compile(preprocessed.clone()).unwrap();
			(name, network, preprocessed, proto_network)
		})
		.collect();

	let mut group = c.benchmark_group("Baseline preprocess");
	for (name, network, _, _) in &demos {
		group.bench_function(**name, |b| b.iter_batched(|| network.clone(), |network| baseline.preprocess(black_box(network)), BatchSize::SmallInput));
	}
	group.finish();

	let mut group = c.benchmark_group("Baseline compile");
	for (name, _, preprocessed, _) in &demos {
		group.bench_function(**name, |b| {
			b.iter_batched(|| preprocessed.clone(), |network| baseline.compile(black_box(network)).unwrap(), BatchSize::SmallInput)
		});
	}
	group.finish();

	let mut group = c.benchmark_group("Baseline executor construction");
	for (name, _, _, proto_network) in &demos {
		group.bench_function(**name, |b| {
			b.iter_batched(
				|| proto_network.clone(),
				|network| baseline.build_executor(black_box(network)).unwrap().tree.len(),
				BatchSize::SmallInput,
			)
		});
	}
	group.finish();

	let mut group = c.benchmark_group("Baseline full pipeline");
	for (name, network, _, _) in &demos {
		group.bench_function(**name, |b| {
			b.iter_batched(|| network.clone(), |network| baseline.run(black_box(network)).unwrap().tree.len(), BatchSize::SmallInput)
		});
	}
	group.finish();
}

fn lean(c: &mut Criterion) {
	use compiler_spike::lean::Lean;
	use compiler_spike::lean::executor::Executor;
	use compiler_spike::workload::load_network;

	let mut lean = Lean::load();
	let demos: Vec<_> = DEMO_ART
		.iter()
		.map(|name| {
			let network = load_network(name);
			let document = lean.convert_document(&network);
			(name, network, document)
		})
		.collect();

	let mut group = c.benchmark_group("Lean document conversion");
	for (name, network, _) in &demos {
		group.bench_function(**name, |b| b.iter(|| lean.convert_document(black_box(network)).definitions.len()));
	}
	group.finish();

	let mut group = c.benchmark_group("Lean compile");
	for (name, _, document) in &demos {
		group.bench_function(**name, |b| b.iter(|| lean.compile(black_box(document)).unwrap().output));
	}
	group.finish();

	let mut group = c.benchmark_group("Lean executor construction");
	for (name, _, document) in &demos {
		let compiled = lean.compile(document).unwrap();
		group.bench_function(**name, |b| b.iter(|| Executor::new(black_box(&compiled)).nodes.len()));
	}
	group.finish();

	let mut group = c.benchmark_group("Lean full pipeline");
	for (name, _, document) in &demos {
		group.bench_function(**name, |b| b.iter(|| Executor::new(&lean.compile(black_box(document)).unwrap()).nodes.len()));
	}
	group.finish();

	let mut group = c.benchmark_group("Lean full pipeline cold selections");
	for (name, _, document) in &demos {
		group.bench_function(**name, |b| {
			b.iter(|| {
				lean.typing.registry.clear_selections();
				Executor::new(&lean.compile(black_box(document)).unwrap()).nodes.len()
			})
		});
	}
	group.finish();
}

criterion_group!(benches, baseline, lean);
criterion_main!(benches);
