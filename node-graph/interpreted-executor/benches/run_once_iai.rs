use graph_craft::util::*;
use graphene_std::Context;
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn setup_run_once(name: &str) -> DynamicExecutor {
	let network = load_from_name(name);
	let proto_network = compile(network);
	futures::executor::block_on(DynamicExecutor::new(proto_network)).unwrap()
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_once)]
pub fn run_once(executor: DynamicExecutor) {
	let context: Context = None;
	black_box(futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), black_box(context))).unwrap());
}

library_benchmark_group!(name = run_once_group; benchmarks = run_once);

main!(library_benchmark_groups = run_once_group);
