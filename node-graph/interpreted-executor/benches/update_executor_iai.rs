use graph_craft::proto::ProtoNetwork;
use graph_craft::util::*;
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};
use interpreted_executor::dynamic_executor::DynamicExecutor;

fn setup_update_executor(name: &str) -> (DynamicExecutor, ProtoNetwork) {
	let network = load_from_name(name);
	let proto_network = compile(network);
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
