mod benchmark_util;

use benchmark_util::scoped_network;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use graph_craft::util::{DEMO_ART, compile};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use preprocessor::Preprocessor;
use std::hint::black_box;

fn compile_pipeline(c: &mut Criterion) {
	let preprocessor = Preprocessor::new();
	let preprocess = |mut network| {
		preprocessor.preprocess(&mut network, &|_| None).unwrap();
		network
	};

	let demos: Vec<_> = DEMO_ART
		.iter()
		.map(|name| {
			let scoped = scoped_network(name);
			let preprocessed = preprocess(scoped.clone());
			let proto_network = compile(preprocessed.clone());
			(name, scoped, preprocessed, proto_network)
		})
		.collect();

	let mut group = c.benchmark_group("Preprocess");
	for (name, scoped, _, _) in &demos {
		group.bench_function(**name, |b| b.iter_batched(|| scoped.clone(), |network| preprocess(black_box(network)), BatchSize::SmallInput));
	}
	group.finish();

	let mut group = c.benchmark_group("Compile");
	for (name, _, preprocessed, _) in &demos {
		group.bench_function(**name, |b| b.iter_batched(|| preprocessed.clone(), |network| compile(black_box(network)), BatchSize::SmallInput));
	}
	group.finish();

	let mut group = c.benchmark_group("Executor construction");
	for (name, _, _, proto_network) in &demos {
		group.bench_function(**name, |b| {
			b.iter_batched(|| proto_network.clone(), |network| DynamicExecutor::new(black_box(network)).unwrap(), BatchSize::SmallInput)
		});
	}
	group.finish();

	let mut group = c.benchmark_group("Full pipeline");
	for (name, scoped, _, _) in &demos {
		group.bench_function(**name, |b| {
			b.iter_batched(
				|| scoped.clone(),
				|network| DynamicExecutor::new(compile(preprocess(black_box(network)))).unwrap(),
				BatchSize::SmallInput,
			)
		});
	}
	group.finish();
}

criterion_group!(benches, compile_pipeline);
criterion_main!(benches);
