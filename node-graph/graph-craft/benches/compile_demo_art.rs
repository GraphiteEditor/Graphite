use criterion::{black_box, criterion_group, criterion_main, Criterion};
use graph_craft::document::NodeNetwork;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;

pub fn compile_to_proto(c: &mut Criterion) {
	let artworks = glob::glob("../../demo-artwork/*.graphite").expect("failed to read glob pattern");
	for path in artworks {
		let Ok(path) = path else { continue };
		let content = std::fs::read(&path).expect("failed to read file");
		let network = load_network(std::str::from_utf8(&content).unwrap());
		let name = path.file_stem().unwrap().to_str().unwrap();

		c.bench_function(name, |b| b.iter_batched(|| network.clone(), |network| compile(black_box(network)), criterion::BatchSize::SmallInput));
	}
}

fn load_network(document_string: &str) -> NodeNetwork {
	let document: serde_json::Value = serde_json::from_str(document_string).expect("Failed to parse document");
	serde_json::from_value::<NodeNetwork>(document["network_interface"]["network"].clone()).expect("Failed to parse document")
}
fn compile(network: NodeNetwork) -> ProtoNetwork {
	let compiler = Compiler {};
	compiler.compile_single(network).unwrap()
}

criterion_group!(benches, compile_to_proto);
criterion_main!(benches);
