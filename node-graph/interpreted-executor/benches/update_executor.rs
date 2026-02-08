mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graph_craft::proto::ProtoNetwork;
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn update_executor(c: &mut Criterion) {
	let mut group = c.benchmark_group("Update Executor");
	bench_for_each_demo(&mut group, |name, g| {
		g.bench_function(name, |b| {
			b.iter_batched(
				|| {
					let (_, proto_network) = setup_network(name);
					let empty = ProtoNetwork::default();
					let executor = futures::executor::block_on(DynamicExecutor::new(empty)).unwrap();
					(executor, proto_network)
				},
				|(mut executor, network)| futures::executor::block_on(executor.update(std::hint::black_box(network))),
				criterion::BatchSize::SmallInput,
			)
		});
	});
	group.finish();
}

criterion_group!(benches, update_executor);
criterion_main!(benches);
