use gpu_compiler as compiler;
use gpu_executor::CompileRequest;
use graph_craft::document::NodeNetwork;
use std::io::Write;

fn main() -> anyhow::Result<()> {
	println!("Starting GPU Compiler!");
	let mut stdin = std::io::stdin();
	let mut stdout = std::io::stdout();
	let compile_dir = std::env::args().nth(1).map(|x| std::path::PathBuf::from(&x)).unwrap_or(tempfile::tempdir()?.into_path());
	let request: CompileRequest = serde_json::from_reader(&mut stdin)?;
	dbg!(&compile_dir);

	let metadata = compiler::Metadata::new("project".to_owned(), vec!["test@example.com".to_owned()]);

	compiler::create_files(&metadata, &request.networks, &compile_dir, &request.io)?;
	let result = compiler::compile(&compile_dir)?;

	let bytes = std::fs::read(result.module.unwrap_single())?;
	// TODO: properly resolve this
	let spirv_path = compile_dir.join("shader.spv");
	std::fs::write(&spirv_path, &bytes)?;

	Ok(())
}
