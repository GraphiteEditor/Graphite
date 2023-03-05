use gpu_compiler as compiler;
use graph_craft::document::NodeNetwork;
use std::io::Write;

fn main() -> anyhow::Result<()> {
	println!("Starting GPU Compiler!");
	let mut stdin = std::io::stdin();
	let mut stdout = std::io::stdout();
	let input_type = std::env::args().nth(1).expect("input type arg missing");
	let output_type = std::env::args().nth(2).expect("output type arg missing");
	let compile_dir = std::env::args().nth(3).map(|x| std::path::PathBuf::from(&x)).unwrap_or(tempfile::tempdir()?.into_path());
	let network: NodeNetwork = serde_json::from_reader(&mut stdin)?;
	let compiler = graph_craft::executor::Compiler {};
	let proto_network = compiler.compile_single(network, true).unwrap();
	dbg!(&compile_dir);

	let metadata = compiler::Metadata::new("project".to_owned(), vec!["test@example.com".to_owned()]);

	compiler::create_files(&metadata, &proto_network, &compile_dir, &input_type, &output_type)?;
	let result = compiler::compile(&compile_dir)?;

	let bytes = std::fs::read(result.module.unwrap_single())?;
	// TODO: properly resolve this
	let spirv_path = compile_dir.join("shader.spv");
	std::fs::write(&spirv_path, &bytes)?;

	Ok(())
}
