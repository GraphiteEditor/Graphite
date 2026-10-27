use compiler_spike::lean::Lean;
use compiler_spike::lean::codegen::{self, Erasure, Evaluation};
use compiler_spike::workload::{DEMO_ART, load_network};
use std::path::PathBuf;
use std::process::Command;

fn type_check(name: &str, source: &str) -> Result<(), String> {
	let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("codegen");
	std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
	let path = directory.join(format!("{name}.rs"));
	std::fs::write(&path, source).map_err(|error| error.to_string())?;

	let result = Command::new("rustc")
		.args(["--edition", "2024", "--crate-type", "lib", "--emit", "metadata", "--out-dir"])
		.arg(&directory)
		.arg(&path)
		.output()
		.map_err(|error| error.to_string())?;
	if result.status.success() {
		Ok(())
	} else {
		Err(String::from_utf8_lossy(&result.stderr).lines().take(40).collect::<Vec<_>>().join("\n"))
	}
}

#[test]
fn erased_emitted_demos_type_check() {
	let mut lean = Lean::load();
	for name in DEMO_ART.into_iter().chain(["rectangle-layers"]) {
		let document = lean.convert_document(&load_network(name));
		let output = lean.compile(&document).unwrap_or_else(|error| panic!("{name} failed to compile: {error}")).output;

		for (erasure, evaluation) in [(Erasure::Bounded, Evaluation::Sync), (Erasure::Everywhere, Evaluation::Async)] {
			let emitted = codegen::emit(&lean, output, erasure, evaluation);
			if let Err(error) = type_check(&format!("{name}-{erasure:?}-{evaluation:?}"), &emitted.source) {
				panic!("{name} ({erasure:?}, {evaluation:?}): emitted source does not type check:\n{error}");
			}
		}
	}
}
