use graph_craft::util::*;
use graphene_std::Context;
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn setup_run_cached(name: &str) -> DynamicExecutor {
	let network = load_from_name(name);
	let proto_network = compile(network);
	let executor = futures::executor::block_on(DynamicExecutor::new(proto_network)).unwrap();

	// Warm up the cache by running once
	let context: Context = None;
	let _ = futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), context.clone()));

	executor
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_cached)]
pub fn run_cached(executor: DynamicExecutor) {
	let context: Context = None;
	black_box(futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), black_box(context))).unwrap());
}

library_benchmark_group!(name = run_cached_group; benchmarks = run_cached);

main!(library_benchmark_groups = run_cached_group);
