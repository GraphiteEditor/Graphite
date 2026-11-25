mod benchmark_util;

use benchmark_util::setup_network;
use graphene_std::application_io;
use gungraun::prelude::*;
use interpreted_executor::dynamic_executor::{DynamicExecutor, OutputHandle};
use std::hint::black_box;

fn setup_run_once(name: &str) -> (DynamicExecutor, OutputHandle) {
	let (executor, output, _) = setup_network(name);
	(executor, output)
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_once)]
pub fn run_once(setup: (DynamicExecutor, OutputHandle)) -> (DynamicExecutor, OutputHandle) {
	let (executor, output) = setup;
	let context = application_io::RenderConfig::default().into_context();
	black_box(executor.tree().eval_tagged_value(output.node(), black_box(&context)).unwrap());

	// Return the executor so its teardown happens outside the measured section
	(executor, output)
}

library_benchmark_group!(name = run_once_group; benchmarks = run_once);

main!(library_benchmark_groups = run_once_group);
