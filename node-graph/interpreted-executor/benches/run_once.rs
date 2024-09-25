use criterion::{criterion_group, criterion_main, Criterion};
use graph_craft::graphene_compiler::Executor;
use graphene_std::transform::Footprint;

mod benchmark_util;
use benchmark_util::{bench_for_each_demo, setup_network};

fn run_once(c: &mut Criterion) {
	let mut group = c.benchmark_group("Run Once");
	let footprint = Footprint::default();
	bench_for_each_demo(&mut group, |name, g| {
		g.bench_function(name, |b| {
			b.iter_batched(
				|| setup_network(name),
				|(executor, _)| futures::executor::block_on((&executor).execute(criterion::black_box(footprint))),
				criterion::BatchSize::SmallInput,
			)
		});
	});
	group.finish();
}

criterion_group!(benches, run_once);
criterion_main!(benches);
