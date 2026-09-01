mod benchmark_util;

use benchmark_util::setup_network;
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io::RenderConfig;
use gungraun::prelude::*;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::hint::black_box;

fn setup_run_cached(name: &str) -> DynamicExecutor {
	let (executor, _) = setup_network(name);

	// Warm up the cache by running once
	let context = RenderConfig::default();
	let _ = Executor::execute(&&executor, context);

	executor
}

#[library_benchmark]
#[benches::with_setup(args = ["changing-seasons", "isometric-fountain", "painted-dreams", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_cached)]
pub fn run_cached(executor: DynamicExecutor) -> DynamicExecutor {
	let context = RenderConfig::default();
	black_box(Executor::execute(&&executor, black_box(context)).unwrap());

	// Return the executor so its teardown happens outside the measured section
	executor
}

library_benchmark_group!(name = run_cached_group; benchmarks = run_cached);

main!(library_benchmark_groups = run_cached_group);
