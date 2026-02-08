mod benchmark_util;

use benchmark_util::setup_network;
use graphene_std::application_io;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::hint::black_box;

fn setup_run_once(name: &str) -> DynamicExecutor {
	let (executor, _) = setup_network(name);
	executor
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_once)]
pub fn run_once(executor: DynamicExecutor) {
	let context = application_io::RenderConfig::default();
	black_box(futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), black_box(context))).unwrap());
}

library_benchmark_group!(name = run_once_group; benchmarks = run_once);

main!(library_benchmark_groups = run_once_group);
