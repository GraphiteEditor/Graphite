mod benchmark_util;

use benchmark_util::setup_network;
use graphene_std::application_io::RenderConfig;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::hint::black_box;

fn setup_run_cached(name: &str) -> DynamicExecutor {
	let (executor, _) = setup_network(name);

	// Warm up the cache by running once
	let context = RenderConfig::default();
	let _ = futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), context));

	executor
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_cached)]
pub fn run_cached(executor: DynamicExecutor) {
	let context = RenderConfig::default();
	black_box(futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), black_box(context))).unwrap());
}

library_benchmark_group!(name = run_cached_group; benchmarks = run_cached);

main!(library_benchmark_groups = run_cached_group);
