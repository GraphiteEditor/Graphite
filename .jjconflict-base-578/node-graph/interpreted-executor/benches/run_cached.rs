mod benchmark_util;

use benchmark_util::{bench_for_each_demo, setup_network};
use criterion::{Criterion, criterion_group, criterion_main};
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io::RenderConfig;

fn subsequent_evaluations(c: &mut Criterion) {
	let mut group = c.benchmark_group("Subsequent Evaluations");
	let context = RenderConfig::default();
	bench_for_each_demo(&mut group, |name, g| {
		let (executor, _) = setup_network(name);
		g.bench_function(name, |b| b.iter(|| Executor::execute(&&executor, std::hint::black_box(context)).unwrap()));
	});
	group.finish();
}

criterion_group!(benches, subsequent_evaluations);
criterion_main!(benches);
