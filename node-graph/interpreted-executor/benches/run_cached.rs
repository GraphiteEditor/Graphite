mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graphene_std::Context;

fn subsequent_evaluations(c: &mut Criterion) {
	let mut group = c.benchmark_group("Subsequent Evaluations");
	let context: Context = None;
	bench_for_each_demo(&mut group, |name, g| {
		let (executor, _) = setup_network(name);
		g.bench_function(name, |b| {
			b.iter(|| futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), criterion::black_box(context.clone()))).unwrap())
		});
	});
	group.finish();
}

criterion_group!(benches, subsequent_evaluations);
criterion_main!(benches);
