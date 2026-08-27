mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io::RenderConfig;
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn run_once(c: &mut Criterion) {
	let mut group = c.benchmark_group("Run Once");
	let context = RenderConfig::default();
	bench_for_each_demo(&mut group, |name, g| {
		let (_, network) = setup_network(name);
		g.bench_function(name, |b| {
			b.iter_batched_ref(
				|| DynamicExecutor::new(network.clone()).unwrap(),
				|executor| Executor::execute(&&*executor, std::hint::black_box(context)).unwrap(),
				criterion::BatchSize::LargeInput,
			)
		});
	});
	group.finish();
}

criterion_group!(benches, run_once);
criterion_main!(benches);
