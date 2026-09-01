mod benchmark_util;

use benchmark_util::setup_network;
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io;
use gungraun::prelude::*;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::hint::black_box;

fn setup_run_once(name: &str) -> DynamicExecutor {
	let (executor, _) = setup_network(name);
	executor
}

#[library_benchmark]
#[benches::with_setup(args = ["changing-seasons", "isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_run_once)]
pub fn run_once(executor: DynamicExecutor) -> (DynamicExecutor, core_types::gpoll::GPoll<graph_craft::document::value::TaggedValue>) {
	let context = application_io::RenderConfig::default();
	let result = black_box(Executor::execute(&&executor, black_box(context)).unwrap());

	// Return the executor and result so their teardown happens outside the measured section
	(executor, result)
}

library_benchmark_group!(name = run_once_group; benchmarks = run_once);

main!(library_benchmark_groups = run_once_group);
