use criterion::{Criterion, black_box, criterion_group, criterion_main};
use graph_craft::util::DEMO_ART;
fn compile_to_proto(c: &mut Criterion) {
	use graph_craft::util::{compile, load_from_name};
	let mut c = c.benchmark_group("Compile Network cold");

	for name in DEMO_ART {
		let network = load_from_name(name);
		c.bench_function(name, |b| b.iter_batched(|| network.clone(), |network| compile(black_box(network)), criterion::BatchSize::SmallInput));
	}
}

criterion_group!(benches, compile_to_proto);
criterion_main!(benches);
