//! Sweeps the synthetic workloads over growing sizes and prints per-stage timings, exposing superlinear stages.
//! Run with `cargo run --release --bin scaling [--memoize | --lean] [shape] [--sizes=a,b,c]`.
//! For `--lean`, the stages are document conversion, lowering, and executor construction, and nodes are unique IR nodes.

use compiler_spike::baseline::Baseline;
use compiler_spike::lean::Lean;
use compiler_spike::lean::executor::Executor;
use compiler_spike::synthetic;
use compiler_spike::workload::Network;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Sizes stop growing once a single pipeline run exceeds this.
const RUN_LIMIT: Duration = Duration::from_secs(2);
const MEASUREMENT_BUDGET: Duration = Duration::from_millis(300);

enum Pipeline {
	Baseline(Baseline),
	Lean(Lean),
}

struct Timings {
	nodes: usize,
	stages: [Duration; 3],
}

fn median(mut samples: Vec<Duration>) -> Duration {
	samples.sort();
	samples[samples.len() / 2]
}

/// Times one run of the pipeline's three stages, returning the stage durations and the node count.
fn run(pipeline: &mut Pipeline, workload: &Network) -> ([Duration; 3], usize) {
	match pipeline {
		Pipeline::Baseline(baseline) => {
			let network = baseline.convert_network(workload);

			let start = Instant::now();
			let preprocessed = baseline.preprocess(network);
			let preprocess = start.elapsed();

			let start = Instant::now();
			let proto_network = baseline.compile(preprocessed).expect("synthetic workloads should compile");
			let compile = start.elapsed();
			let nodes = proto_network.nodes.len();

			let start = Instant::now();
			let executor = baseline.build_executor(proto_network).expect("synthetic workloads should type check");
			black_box(executor.tree.len());
			([preprocess, compile, start.elapsed()], nodes)
		}
		Pipeline::Lean(lean) => {
			let start = Instant::now();
			let document = lean.convert_document(workload);
			let conversion = start.elapsed();

			let start = Instant::now();
			let compiled = lean.compile(&document).expect("synthetic workloads should compile");
			let lowering = start.elapsed();

			let start = Instant::now();
			black_box(Executor::new(&compiled).nodes.len());
			([conversion, lowering, start.elapsed()], compiled.ir.len())
		}
	}
}

fn measure(pipeline: &mut Pipeline, workload: &Network) -> Timings {
	let mut samples: [Vec<Duration>; 3] = Default::default();
	let mut nodes = 0;

	let started = Instant::now();
	while samples[0].is_empty() || (started.elapsed() < MEASUREMENT_BUDGET && samples[0].len() < 50) {
		let (stages, node_count) = run(pipeline, workload);
		for (stage_samples, stage) in samples.iter_mut().zip(stages) {
			stage_samples.push(stage);
		}
		nodes = node_count;
	}

	Timings { nodes, stages: samples.map(median) }
}

fn sweep(pipeline: &mut Pipeline, shape: &str, sizes: &[usize], generate: impl Fn(usize) -> Network) {
	let stage_names = match pipeline {
		Pipeline::Baseline(_) => ["preprocess", "compile", "executor"],
		Pipeline::Lean(_) => ["conversion", "lowering", "executor"],
	};
	println!("\n{shape}");
	println!(
		"{:>8} {:>8} {:>8} {:>11} {:>11} {:>11} {:>11} {:>10} {:>9}",
		"size", "document", "nodes", stage_names[0], stage_names[1], stage_names[2], "total", "ns/node", "ns/doc"
	);
	for &size in sizes {
		let workload = generate(size);
		let document_nodes = count_nodes(&workload);
		let timings = measure(pipeline, &workload);
		let [first, second, third] = timings.stages;
		// Lean's document conversion stands in for a representation the editor would already hold, so it is excluded from the total
		let total = match pipeline {
			Pipeline::Baseline(_) => first + second + third,
			Pipeline::Lean(_) => second + third,
		};
		println!(
			"{size:>8} {document_nodes:>8} {:>8} {first:>11.3?} {second:>11.3?} {third:>11.3?} {total:>11.3?} {:>10.0} {:>9.0}",
			timings.nodes,
			total.as_nanos() as f64 / timings.nodes.max(1) as f64,
			total.as_nanos() as f64 / document_nodes.max(1) as f64
		);
		if total > RUN_LIMIT {
			println!("{:>8} (stopping: a run exceeded {RUN_LIMIT:?})", "");
			break;
		}
	}
}

fn count_nodes(network: &Network) -> usize {
	network
		.nodes
		.iter()
		.map(|(_, node)| match &node.implementation {
			compiler_spike::workload::Implementation::Network(inner) => 1 + count_nodes(inner),
			_ => 1,
		})
		.sum()
}

fn main() {
	let arguments: Vec<String> = std::env::args().skip(1).collect();
	let memoize = arguments.iter().any(|argument| argument == "--memoize");
	let filter = arguments.iter().find(|argument| !argument.starts_with("--"));
	let selected = |shape: &str| filter.is_none_or(|filter| shape.starts_with(filter.as_str()));
	let size_override: Option<Vec<usize>> = arguments
		.iter()
		.find_map(|argument| argument.strip_prefix("--sizes="))
		.map(|sizes| sizes.split(',').map(|size| size.parse().expect("--sizes takes comma-separated numbers")).collect());
	let sizes = |default: &'static [usize]| size_override.clone().unwrap_or_else(|| default.to_vec());

	let mut baseline = match arguments.iter().any(|argument| argument == "--lean") {
		true => Pipeline::Lean(Lean::load()),
		false => {
			let mut baseline = Baseline::load();
			baseline.memoize_context_analysis = memoize;
			Pipeline::Baseline(baseline)
		}
	};

	if selected("rectangles") {
		sweep(&mut baseline, "rectangles(layers)", &sizes(&[1, 10, 50, 100, 200, 400, 800, 1600]), synthetic::rectangle_layers);
	}
	if selected("chain") {
		sweep(&mut baseline, "chain(length)", &sizes(&[100, 200, 400, 800, 1600, 3200, 6400]), synthetic::chain);
	}
	if selected("fan_out") {
		sweep(&mut baseline, "fan_out(width)", &sizes(&[100, 200, 400, 800, 1600, 3200, 6400]), synthetic::fan_out);
	}
	if selected("lattice") {
		sweep(&mut baseline, "lattice(width 8, depth)", &sizes(&[2, 4, 6, 8, 10, 12, 14, 16, 18, 20]), |depth| {
			synthetic::lattice(8, depth)
		});
		sweep(&mut baseline, "lattice(width, depth 4)", &sizes(&[8, 16, 32, 64, 128, 256, 512]), |width| synthetic::lattice(width, 4));
	}
	if selected("nesting") {
		sweep(&mut baseline, "nesting(depth, chain 10)", &sizes(&[1, 2, 4, 8, 16, 32, 64, 128]), |depth| synthetic::nesting(depth, 10));
	}
	if selected("instances") {
		sweep(&mut baseline, "instances(count, chain 20)", &sizes(&[10, 20, 40, 80, 160, 320]), |count| {
			synthetic::instances(count, 20)
		});
	}
}
