use graph_craft::document::NodeNetwork;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;
use std::path::PathBuf;

#[cfg(feature = "criterion")]
use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(feature = "iai")]
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};

fn load_network(document_string: &str) -> NodeNetwork {
	let document: serde_json::Value = serde_json::from_str(document_string).expect("Failed to parse document");
	serde_json::from_value::<NodeNetwork>(document["network_interface"]["network"].clone()).expect("Failed to parse document")
}

fn compile(network: NodeNetwork) -> ProtoNetwork {
	let compiler = Compiler {};
	compiler.compile_single(network).unwrap()
}

fn bench_compile(path: PathBuf) {
	let content = std::fs::read(&path).expect("failed to read file");
	let network = load_network(std::str::from_utf8(&content).unwrap());
	black_box(compile(black_box(network)));
}

#[cfg(feature = "criterion")]
fn compile_to_proto(c: &mut Criterion) {
	let artworks = glob::glob("../../demo-artwork/*.graphite").expect("failed to read glob pattern");
	for path in artworks {
		let Ok(path) = path else { continue };
		let name = path.file_stem().unwrap().to_str().unwrap();
		c.bench_function(name, |b| b.iter(|| bench_compile(path.clone())));
	}
}

#[cfg_attr(feature = "iai", library_benchmark)]
fn iai_compile_to_proto() {
	let artworks = glob::glob("../../demo-artwork/*.graphite").expect("failed to read glob pattern");
	for path in artworks {
		let Ok(path) = path else { continue };
		bench_compile(path);
	}
}

#[cfg(feature = "criterion")]
criterion_group!(benches, compile_to_proto);

#[cfg(feature = "criterion")]
criterion_main!(benches);

#[cfg(feature = "iai")]
library_benchmark_group!(name = compile_group; benchmarks = iai_compile_to_proto);

#[cfg(feature = "iai")]
main!(library_benchmark_groups = compile_group);
