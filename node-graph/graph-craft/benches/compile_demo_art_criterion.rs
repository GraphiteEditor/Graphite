use criterion::{Criterion, black_box, criterion_group, criterion_main};
use graph_craft::util::DEMO_ART;
fn compile_to_proto(c: &mut Criterion) {
	use graph_craft::util::load_from_name;
	let mut c = c.benchmark_group("Compile Network cold");

	for name in DEMO_ART {
		let network = load_from_name(name);
		c.bench_function(name, |b: &mut criterion::Bencher<'_>| {
			b.iter_batched(|| network.clone(), |mut network| black_box(network.compile()), criterion::BatchSize::SmallInput)
		});
	}
}

criterion_group!(benches, compile_to_proto);
criterion_main!(benches);
