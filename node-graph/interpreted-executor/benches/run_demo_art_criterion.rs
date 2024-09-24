use criterion::{black_box, criterion_group, criterion_main, measurement::Measurement, BenchmarkGroup, Criterion};
use graph_craft::{
	proto::ProtoNetwork,
	util::{compile, load_from_name, DEMO_ART},
};
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn update_executor<M: Measurement>(name: &str, c: &mut BenchmarkGroup<M>) {
	let network = load_from_name(name);
	let proto_network = compile(network);
	let empty = ProtoNetwork::default();

	let executor = futures::executor::block_on(DynamicExecutor::new(empty)).unwrap();

	c.bench_function(name, |b| {
		b.iter_batched(
			|| (executor.clone(), proto_network.clone()),
			|(mut executor, network)| futures::executor::block_on(executor.update(black_box(network))),
			criterion::BatchSize::SmallInput,
		)
	});
}

fn update_executor_demo(c: &mut Criterion) {
	let mut g = c.benchmark_group("Update Executor");
	for name in DEMO_ART {
		update_executor(name, &mut g);
	}
}

criterion_group!(benches, update_executor_demo);
criterion_main!(benches);
