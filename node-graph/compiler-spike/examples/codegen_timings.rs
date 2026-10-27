//! Usage: `cargo run --release --example codegen_timings [--iterations=N] [--async] [--modes=Everywhere,Bounded,Monomorphic,Functions,FunctionPointers] [demo...]`

use compiler_spike::lean::Lean;
use compiler_spike::lean::codegen::{self, Erasure, Evaluation};
use compiler_spike::workload::{DEMO_ART, load_network};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const RUSTC_TIMEOUT_SECONDS: &str = "600";

fn build(path: &Path) -> Option<Duration> {
	let start = Instant::now();
	let status = Command::new("timeout")
		.args([RUSTC_TIMEOUT_SECONDS, "rustc", "--edition", "2024", "-C", "opt-level=3", "-C", "codegen-units=1", "-o"])
		.arg(path.with_extension(""))
		.arg(path)
		.output()
		.ok()?
		.status;
	status.success().then(|| start.elapsed())
}

/// Returns the nanoseconds per evaluation and the checksum.
fn run(path: &Path, iterations: u64) -> Option<(u64, String)> {
	let output = Command::new(path.with_extension("")).arg(iterations.to_string()).output().ok()?;
	let stdout = String::from_utf8(output.stdout).ok()?;
	let (nanoseconds, checksum) = stdout.trim().split_once(' ')?;
	Some((nanoseconds.parse().ok()?, checksum.to_string()))
}

#[derive(Clone, Copy, Debug)]
enum Variant {
	Structs(Erasure, Evaluation),
	Functions,
	FunctionPointers,
}

impl Variant {
	fn label(self) -> String {
		match self {
			Variant::Structs(erasure, evaluation) => format!("{erasure:?} {evaluation:?}"),
			Variant::Functions => "Functions".to_string(),
			Variant::FunctionPointers => "FunctionPointers".to_string(),
		}
	}

	fn parse(label: &str, asynchronous: bool) -> Vec<Variant> {
		let evaluations = if asynchronous { vec![Evaluation::Sync, Evaluation::Async] } else { vec![Evaluation::Sync] };
		let structs = |erasure| evaluations.iter().map(|&evaluation| Variant::Structs(erasure, evaluation)).collect();
		match label {
			"Everywhere" => structs(Erasure::Everywhere),
			"Bounded" => structs(Erasure::Bounded),
			"Monomorphic" => structs(Erasure::Monomorphic),
			"Functions" => vec![Variant::Functions],
			"FunctionPointers" => vec![Variant::FunctionPointers],
			_ => panic!("Unknown variant {label}"),
		}
	}
}

fn main() {
	let mut iterations = 10_000;
	let mut labels = "Everywhere,Bounded,Monomorphic,Functions,FunctionPointers".to_string();
	let mut asynchronous = false;
	let mut selected = Vec::new();
	for argument in std::env::args().skip(1) {
		if let Some(value) = argument.strip_prefix("--iterations=") {
			iterations = value.parse().expect("--iterations should be a number");
		} else if let Some(value) = argument.strip_prefix("--modes=") {
			labels = value.to_string();
		} else if argument == "--async" {
			asynchronous = true;
		} else {
			selected.push(argument);
		}
	}
	let variants: Vec<Variant> = labels.split(',').flat_map(|label| Variant::parse(label, asynchronous)).collect();
	let names: Vec<&str> = DEMO_ART
		.into_iter()
		.chain(["rectangle-layers"])
		.filter(|name| selected.is_empty() || selected.iter().any(|selected| selected == name))
		.collect();
	let directory = std::env::temp_dir().join("compiler-spike-codegen");
	std::fs::create_dir_all(&directory).unwrap();

	let mut lean = Lean::load();
	println!(
		"{:<26} {:<18} {:>6} {:>10} {:>9} {:>10} {:>12} {:>9}",
		"document", "variant", "nodes", "node evals", "emit", "build", "ns per run", "checksum"
	);
	for name in names {
		let document = lean.convert_document(&load_network(name));
		let output = lean.compile(&document).unwrap().output;

		for &variant in &variants {
			let start = Instant::now();
			let emitted = match variant {
				Variant::Structs(erasure, evaluation) => codegen::emit(&lean, output, erasure, evaluation),
				Variant::Functions => codegen::emit_functions(&lean, output, false),
				Variant::FunctionPointers => codegen::emit_functions(&lean, output, true),
			};
			let emit_time = start.elapsed();

			let path = directory.join(format!("{name}-{}.rs", variant.label().replace(' ', "-")));
			std::fs::write(&path, &emitted.source).unwrap();
			let build_time = build(&path);
			let result = build_time.and_then(|_| run(&path, iterations));

			println!(
				"{name:<26} {:<18} {:>6} {:>10.0} {:>9.2?} {:>10} {:>12} {:>9}",
				variant.label(),
				emitted.statements,
				emitted.evaluations_per_run,
				emit_time,
				build_time.map_or("failed".to_string(), |time| format!("{:.2}s", time.as_secs_f64())),
				result.as_ref().map_or("failed".to_string(), |(nanoseconds, _)| nanoseconds.to_string()),
				result.as_ref().map_or("-".to_string(), |(_, checksum)| checksum.chars().take(8).collect()),
			);
		}
	}
}
