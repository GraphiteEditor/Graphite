use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use document_loader::DEMO_ARTWORK;

fn compile_to_proto(c: &mut Criterion) {
	use document_loader::load_demo_artwork;
	use graph_craft::util::compile;
	let mut c = c.benchmark_group("Compile Network cold");

	for name in DEMO_ARTWORK {
		let network = load_demo_artwork(name);
		c.bench_function(name, |b| b.iter_batched(|| network.clone(), |network| compile(black_box(network)), criterion::BatchSize::SmallInput));
	}
}

criterion_group!(benches, compile_to_proto);
criterion_main!(benches);
