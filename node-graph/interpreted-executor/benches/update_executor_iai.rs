mod benchmark_util;

use benchmark_util::setup_network;
use graph_craft::proto::ProtoNetwork;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::hint::black_box;

fn setup_update_executor(name: &str) -> (DynamicExecutor, ProtoNetwork) {
	let (_, proto_network) = setup_network(name);
	let empty = ProtoNetwork::default();
	let executor = futures::executor::block_on(DynamicExecutor::new(empty)).unwrap();
	(executor, proto_network)
}

#[library_benchmark]
#[benches::with_setup(args = ["isometric-fountain", "painted-dreams", "procedural-string-lights", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_update_executor)]
pub fn update_executor(setup: (DynamicExecutor, ProtoNetwork)) {
	let (mut executor, network) = setup;
	let _ = black_box(futures::executor::block_on(executor.update(black_box(network))));
}

library_benchmark_group!(name = update_group; benchmarks = update_executor);

main!(library_benchmark_groups = update_group);
