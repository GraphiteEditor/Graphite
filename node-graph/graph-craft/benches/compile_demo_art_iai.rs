use graph_craft::document::NodeNetwork;
use graph_craft::util::*;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = load_from_name)]
pub fn compile_to_proto(_input: NodeNetwork) {
	std::hint::black_box(compile(_input));
}

library_benchmark_group!(name = compile_group; benchmarks = compile_to_proto);

main!(library_benchmark_groups = compile_group);
