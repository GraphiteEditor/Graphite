use std::io::Write;

pub fn compile_spirv(network: &graph_craft::document::NodeNetwork, input_type: &str, output_type: &str, compile_dir: Option<&str>) -> anyhow::Result<Vec<u8>> {
	let serialized_graph = serde_json::to_string(&network)?;
	let features = "";
	#[cfg(feature = "profiling")]
	let features = "profiling";

	let mut carog_command = std::process::Command::new("cargo")
		.arg("run")
		.arg("--release")
		.arg("--target-dir")
		.arg(compile_dir.unwrap_or("target"))
		.arg("--manifest-path")
		.current_dir(std::env::var("CARGO_MANIFEST_DIR").unwrap() + "/../Carog.toml")
		.arg("--features")
		.arg(features)
		.stdin(std::process::Stdio::piped())
		.spawn()?;
	carog_command.stdin.as_mut().unwrap().write_all(serialized_graph.as_bytes())?;
	let output = carog_command.wait_with_output()?;
	if !output.status.success() {
		return Err(anyhow::anyhow!("carog failed: {}", String::from_utf8_lossy(&output.stderr)));
	}
	Ok(output.stdout)
}
