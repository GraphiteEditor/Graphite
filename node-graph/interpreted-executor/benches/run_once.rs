mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graphene_std::application_io::RenderConfig;

fn run_once(c: &mut Criterion) {
	let mut group = c.benchmark_group("Run Once");
	let context = RenderConfig::default();
	bench_for_each_demo(&mut group, |name, g| {
		g.bench_function(name, |b| {
			b.iter_batched(
				|| setup_network(name),
				|(executor, _)| futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), std::hint::black_box(context))).unwrap(),
				criterion::BatchSize::SmallInput,
			)
		});
	});
	group.finish();
}

criterion_group!(benches, run_once);
criterion_main!(benches);
