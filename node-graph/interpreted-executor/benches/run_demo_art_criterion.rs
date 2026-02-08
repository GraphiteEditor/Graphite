use criterion::measurement::Measurement;
use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main};
use graph_craft::graphene_compiler::Executor;
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::{DEMO_ART, compile, load_from_name};
use graphene_std::transform::Footprint;
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn update_executor<M: Measurement>(name: &str, c: &mut BenchmarkGroup<M>) {
	let network = load_from_name(name);
	let proto_network = compile(network);
	let empty = ProtoNetwork::default();

	let executor = futures::executor::block_on(DynamicExecutor::new(empty)).unwrap();

	c.bench_function(name, |b| {
		b.iter_batched(
			|| (executor.clone(), proto_network.clone()),
			|(mut executor, network)| futures::executor::block_on(executor.update(std::hint::black_box(network))),
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

fn run_once<M: Measurement>(name: &str, c: &mut BenchmarkGroup<M>) {
	let network = load_from_name(name);
	let proto_network = compile(network);

	let executor = futures::executor::block_on(DynamicExecutor::new(proto_network)).unwrap();
	let footprint = Footprint::default();

	c.bench_function(name, |b| b.iter(|| futures::executor::block_on((&executor).execute(footprint))));
}
fn run_once_demo(c: &mut Criterion) {
	let mut g = c.benchmark_group("Run Once no render");
	for name in DEMO_ART {
		run_once(name, &mut g);
	}
}

criterion_group!(benches, update_executor_demo, run_once_demo);
criterion_main!(benches);
