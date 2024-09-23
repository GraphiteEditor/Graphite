use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn compile_to_proto(c: &mut Criterion) {
	use graph_craft::util::{compile, load_from_name};

	let artworks = glob::glob("../../demo-artwork/*.graphite").expect("failed to read glob pattern");
	for path in artworks {
		let Ok(path) = path else { continue };
		let name = path.file_stem().unwrap().to_str().unwrap();
		let network = load_from_name(name);
		c.bench_function(name, |b| b.iter_batched(|| network.clone(), |network| compile(black_box(network)), criterion::BatchSize::SmallInput));
	}
}

criterion_group!(benches, compile_to_proto);
criterion_main!(benches);
