mod benchmark_util;

use benchmark_util::setup_network;
use graphene_std::application_io::RenderConfig;
use gungraun::prelude::*;
use interpreted_executor::dynamic_executor::{DynamicExecutor, OutputHandle};
use std::hint::black_box;

fn setup_run_cached(name: &str) -> (DynamicExecutor, OutputHandle) {
	let (executor, output, _) = setup_network(name);

	// Warm up the cache by running once
	let context = RenderConfig::default().into_context();
	let _ = executor.tree().eval_tagged_value(output.node(), &context);

	(executor, output)
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_cached)]
pub fn run_cached(setup: (DynamicExecutor, OutputHandle)) -> (DynamicExecutor, OutputHandle) {
	let (executor, output) = setup;
	let context = RenderConfig::default().into_context();
	black_box(executor.tree().eval_tagged_value(output.node(), black_box(&context)).unwrap());

	// Return the executor so its teardown happens outside the measured section
	(executor, output)
}

library_benchmark_group!(name = run_cached_group; benchmarks = run_cached);

main!(library_benchmark_groups = run_cached_group);
