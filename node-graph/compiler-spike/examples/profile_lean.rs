use compiler_spike::lean::Lean;
use compiler_spike::lean::executor::Executor;
use compiler_spike::workload::load_network;

fn main() {
	let name = std::env::args().nth(1).unwrap_or_else(|| "red-dress".to_string());
	let mut lean = Lean::load();
	let document = lean.convert_document(&load_network(&name));
	for _ in 0..3000 {
		let compiled = lean.compile(&document).unwrap();
		std::hint::black_box(Executor::new(&compiled).nodes.len());
	}
}
