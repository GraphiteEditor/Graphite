mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graphene_std::application_io::RenderConfig;

fn subsequent_evaluations(c: &mut Criterion) {
	let mut group = c.benchmark_group("Subsequent Evaluations");
	let context = RenderConfig::default();
	bench_for_each_demo(&mut group, |name, g| {
		let (executor, _) = setup_network(name);
		g.bench_function(name, |b| {
			b.iter(|| futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), std::hint::black_box(context))).unwrap())
		});
	});
	group.finish();
}

criterion_group!(benches, subsequent_evaluations);
criterion_main!(benches);
