use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use graph_craft::util::DEMO_ART;
use std::path::Path;

fn compile_to_proto(c: &mut Criterion) {
	use graph_craft::util::compile;
	let mut c = c.benchmark_group("Compile Network cold");

	for name in DEMO_ART {
		let network = futures::executor::block_on(document_loader::open_document(Path::new(&format!("../../demo-artwork/{name}.graphite"))))
			.unwrap()
			.network;
		c.bench_function(name, |b| b.iter_batched(|| network.clone(), |network| compile(black_box(network)), criterion::BatchSize::SmallInput));
	}
}

criterion_group!(benches, compile_to_proto);
criterion_main!(benches);
