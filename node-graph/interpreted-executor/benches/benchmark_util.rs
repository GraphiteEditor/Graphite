use criterion::BenchmarkGroup;
use criterion::measurement::Measurement;
use futures::executor::block_on;
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::{DEMO_ART, compile, load_from_name};
use interpreted_executor::dynamic_executor::DynamicExecutor;

pub fn setup_network(name: &str) -> (DynamicExecutor, ProtoNetwork) {
	let network = load_from_name(name);
	let proto_network = compile(network);
	let executor = block_on(DynamicExecutor::new(proto_network.clone())).unwrap();
	(executor, proto_network)
}

pub fn bench_for_each_demo<M: Measurement, F>(group: &mut BenchmarkGroup<M>, f: F)
where
	F: Fn(&str, &mut BenchmarkGroup<M>),
{
	for name in DEMO_ART {
		f(name, group);
	}
}
