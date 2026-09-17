use core_types::transform::Footprint;
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::graphene_compiler::Executor;
use graph_craft::util::{compile, load_network};
use graphene_std::application_io::{EditorApi, RenderConfig};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::time::Instant;

fn main() {
	let path = std::env::args().nth(1).expect("path to a .graphite document");
	let document = std::fs::read_to_string(&path).expect("readable document");
	let network = load_network(&document);
	let editor_api = std::sync::Arc::new(EditorApi::default());
	let mut network = wrap_network_in_scope(network, editor_api);
	preprocessor::Preprocessor::new().preprocess(&mut network, &|_| None).expect("preprocess");
	let proto = compile(network, &interpreted_executor::node_registry::NODE_REGISTRY);
	let executor = DynamicExecutor::new(proto).expect("executor");

	let viewport = |transform: DAffine2| Footprint {
		transform,
		resolution: UVec2::new(1024, 1024),
		..Footprint::default()
	};
	let config = |transform: DAffine2| RenderConfig {
		viewport: viewport(transform),
		..RenderConfig::default()
	};
	let base = config(DAffine2::IDENTITY);
	let panned = config(DAffine2::from_translation(DVec2::new(100., 50.)));
	let zoomed = config(DAffine2::from_scale(DVec2::splat(2.)));

	let steps = [
		("cold", base),
		("warm (same view)", base),
		("pan", panned),
		("pan again", panned),
		("zoom", zoomed),
		("zoom again", zoomed),
		("back to base", base),
		("zoom", zoomed),
	];
	// PROBE_COLD_REPEATS=N re-runs the base view N times behind a flush, so a
	// profile of this process is dominated by cold frames rather than by loading.
	if let Some(repeats) = std::env::var("PROBE_COLD_REPEATS").ok().and_then(|n| n.parse::<usize>().ok()) {
		std::hint::black_box(Executor::execute(&&executor, base).expect("execute"));
		for _ in 0..repeats {
			executor.flush_persistent();
			let started = Instant::now();
			std::hint::black_box(Executor::execute(&&executor, base).expect("execute"));
			println!("{:>18}: {:>10.3?}", "cold (flushed)", started.elapsed());
			#[cfg(debug_assertions)]
			if std::env::var_os("GRAPHENE_BATCH_DEBUG").is_some() {
				let mut rows = core_types::record::take_batch_tally();
				rows.sort_by_key(|(_, (_, _, lanes))| std::cmp::Reverse(*lanes));
				let (mut m, mut u, mut l) = (0, 0, 0);
				for (name, (batched, unbatched, lanes)) in &rows {
					m += batched;
					u += unbatched;
					l += lanes;
					println!("  tally {batched:>7} batched {unbatched:>7} unbatched {lanes:>9} lanes  {name}");
				}
				println!("  tally {m:>7} batched {u:>7} unbatched {l:>9} lanes  TOTAL");
			}
		}
		return;
	}
	for (name, config) in steps {
		let started = Instant::now();
		let result = Executor::execute(&&executor, config).expect("execute");
		println!("{name:>18}: {:>10.3?}", started.elapsed());
		std::hint::black_box(result);
	}
}
