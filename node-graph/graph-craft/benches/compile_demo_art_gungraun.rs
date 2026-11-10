use graph_craft::document::NodeNetwork;
use graph_craft::util::*;
use gungraun::prelude::*;
use std::path::Path;

fn open_demo_artwork(name: &str) -> NodeNetwork {
	futures::executor::block_on(document_loader::open_document(Path::new(&format!("../../demo-artwork/{name}.graphite"))))
		.unwrap()
		.network
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = open_demo_artwork)]
pub fn compile_to_proto(_input: NodeNetwork) {
	std::hint::black_box(compile(_input));
}

library_benchmark_group!(name = compile_group; benchmarks = compile_to_proto);

main!(library_benchmark_groups = compile_group);
