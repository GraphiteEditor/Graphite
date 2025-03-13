mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graph_craft::graphene_compiler::Executor;
use graphene_std::transform::Footprint;

fn subsequent_evaluations(c: &mut Criterion) {
	let mut group = c.benchmark_group("Subsequent Evaluations");
	let footprint = Footprint::default();
	bench_for_each_demo(&mut group, |name, g| {
		let (executor, _) = setup_network(name);
		futures::executor::block_on((&executor).execute(criterion::black_box(footprint))).unwrap();
		g.bench_function(name, |b| b.iter(|| futures::executor::block_on((&executor).execute(criterion::black_box(footprint)))));
	});
	group.finish();
}

criterion_group!(benches, subsequent_evaluations);
criterion_main!(benches);
