use criterion::BenchmarkGroup;
use criterion::measurement::Measurement;
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::{DEMO_ART, compile, load_from_name};
use graphene_std::application_io::EditorApi;
use graphene_std::runtime::{AsyncRuntime, Clock, Spawn, Task};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;

pub fn setup_network(name: &str) -> (DynamicExecutor, ProtoNetwork) {
	let network = load_from_name(name);
	let editor_api = std::sync::Arc::new(EditorApi::default());
	let mut network = wrap_network_in_scope(
		network,
		editor_api,
		AsyncRuntime::new(std::sync::Arc::new(DummySpawner), std::sync::Arc::new(InstantClock(std::time::Instant::now()))).handle(),
	);
	let preprocessor = preprocessor::Preprocessor::new();
	preprocessor.preprocess(&mut network, &|_| None).unwrap();
	let proto_network = compile(network);
	let executor = DynamicExecutor::new(proto_network.clone()).unwrap();
	(executor, proto_network)
}

#[allow(dead_code, reason = "shared by the bench targets, only some of which call it, and each compiles this module separately")]
pub fn bench_for_each_demo<M: Measurement, F>(group: &mut BenchmarkGroup<M>, f: F)
where
	F: Fn(&str, &mut BenchmarkGroup<M>),
{
	for name in DEMO_ART {
		f(name, group);
	}
}

// TODO: Consider replacing this with a real spawner and benchmarking async nodes
struct DummySpawner;
impl Spawn for DummySpawner {
	fn spawn(&self, _task: Task) {}
}

/// Time since the clock was made.
struct InstantClock(std::time::Instant);
impl Clock for InstantClock {
	fn now(&self) -> std::time::Duration {
		self.0.elapsed()
	}
}
