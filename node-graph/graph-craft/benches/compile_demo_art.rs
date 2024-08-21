use graph_craft::document::NodeNetwork;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;

#[cfg(feature = "criterion")]
use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(all(not(feature = "criterion"), feature = "iai"))]
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};

fn load_network(document_string: &str) -> NodeNetwork {
	let document: serde_json::Value = serde_json::from_str(document_string).expect("Failed to parse document");
	serde_json::from_value::<NodeNetwork>(document["network_interface"]["network"].clone()).expect("Failed to parse document")
}

fn compile(network: NodeNetwork) -> ProtoNetwork {
	let compiler = Compiler {};
	compiler.compile_single(network).unwrap()
}

#[cfg(all(not(feature = "criterion"), feature = "iai"))]
fn load_from_name(name: &str) -> NodeNetwork {
	let content = std::fs::read(&format!("../../demo-artwork/{name}.graphite")).expect("failed to read file");
	let network = load_network(std::str::from_utf8(&content).unwrap());
	let content = std::str::from_utf8(&content).unwrap();
	black_box(compile(black_box(network)));
	load_network(content)
}

#[cfg(feature = "criterion")]
fn compile_to_proto(c: &mut Criterion) {
	let artworks = glob::glob("../../demo-artwork/*.graphite").expect("failed to read glob pattern");
	for path in artworks {
		let Ok(path) = path else { continue };
		let name = path.file_stem().unwrap().to_str().unwrap();
		let content = std::fs::read(&path).expect("failed to read file");
		let network = load_network(std::str::from_utf8(&content).unwrap());
		c.bench_function(name, |b| b.iter_batched(|| network.clone(), |network| compile(black_box(network)), criterion::BatchSize::SmallInput));
	}
}

#[cfg_attr(all(feature = "iai", not(feature = "criterion")), library_benchmark)]
#[cfg_attr(all(feature = "iai", not(feature="criterion")), benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "red-dress", "valley-of-spires"], setup = load_from_name))]
pub fn iai_compile_to_proto(_input: NodeNetwork) {
	#[cfg(all(feature = "iai", not(feature = "criterion")))]
	black_box(compile(_input));
}

#[cfg(feature = "criterion")]
criterion_group!(benches, compile_to_proto);

#[cfg(feature = "criterion")]
criterion_main!(benches);

#[cfg(all(not(feature = "criterion"), feature = "iai"))]
library_benchmark_group!(name = compile_group; benchmarks = iai_compile_to_proto);

#[cfg(all(not(feature = "criterion"), feature = "iai"))]
main!(library_benchmark_groups = compile_group);

#[cfg(all(not(feature = "criterion"), not(feature = "iai")))]
fn main() {}
