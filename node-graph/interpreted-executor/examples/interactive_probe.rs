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
		let first = Executor::execute(&&executor, base).expect("execute");
		println!("{:>18}: digest={}", "cold", digest(&first));
		for _ in 0..repeats {
			executor.flush_persistent();
			let started = Instant::now();
			let result = Executor::execute(&&executor, base).expect("execute");
			let elapsed = started.elapsed();
			println!("{:>18}: {:>10.3?}  digest={}", "cold (flushed)", elapsed, digest(&result));
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
				let mut kernels = core_types::record::take_kernel_tally();
				kernels.sort_by_key(|(_, (calls, _))| std::cmp::Reverse(*calls));
				for ((kernel, how), (calls, lanes)) in &kernels {
					println!("  kernel {calls:>8} calls {lanes:>9} lanes  {kernel:<28} {how}");
				}
			}
		}
		return;
	}
	for (name, config) in steps {
		let started = Instant::now();
		let result = Executor::execute(&&executor, config).expect("execute");
		let elapsed = started.elapsed();
		println!("{name:>18}: {:>10.3?}  digest={}", elapsed, digest(&result));
	}
}

/// A stable digest of a frame's output, so two frames can be compared for identical results.
fn digest<T: std::fmt::Debug>(value: &T) -> String {
	use std::hash::{Hash, Hasher};
	let text = format!("{value:?}");
	// Pattern ids are fresh uuids per render, so hex runs of twelve or more are masked before hashing.
	let mut masked = String::with_capacity(text.len());
	let mut run = String::new();
	for c in text.chars().chain(std::iter::once(' ')) {
		if c.is_ascii_hexdigit() {
			run.push(c);
			continue;
		}
		if run.len() >= 12 {
			masked.push('#');
		} else {
			masked.push_str(&run);
		}
		run.clear();
		masked.push(c);
	}
	let text = masked;
	if text.len() < 400 {
		eprintln!("  result: {text}");
	}
	let mut hasher = std::hash::DefaultHasher::new();
	text.hash(&mut hasher);
	format!("{:016x} ({} bytes)", hasher.finish(), text.len())
}
