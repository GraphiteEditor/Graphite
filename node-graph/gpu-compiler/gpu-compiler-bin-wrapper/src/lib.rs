use graph_craft::{proto::ProtoNetwork, Type};
use serde::{Deserialize, Serialize};
use std::io::Write;

pub fn compile_spirv(request: &CompileRequest, compile_dir: Option<&str>, manifest_path: &str) -> anyhow::Result<Vec<u8>> {
	let serialized_graph = serde_json::to_string(request)?;
	let features = "";
	#[cfg(feature = "profiling")]
	let features = "profiling";

	println!("calling cargo run!");
	let non_cargo_env_vars = std::env::vars().filter(|(k, _)| k.starts_with("PATH")).collect::<Vec<_>>();
	let mut cargo_command = std::process::Command::new("/usr/bin/cargo")
		.arg("run")
		.arg("--release")
		.arg("--manifest-path")
		.arg(manifest_path)
		.current_dir(manifest_path.replace("Cargo.toml", ""))
		.env_clear()
		.envs(non_cargo_env_vars)
		.arg("--features")
		.arg(features)
		// TODO: handle None case properly
		.arg(compile_dir.unwrap())
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?;

	cargo_command.stdin.as_mut().unwrap().write_all(serialized_graph.as_bytes())?;
	let output = cargo_command.wait_with_output()?;
	if !output.status.success() {
		return Err(anyhow::anyhow!("cargo failed: {}", String::from_utf8_lossy(&output.stderr)));
	}
	Ok(std::fs::read(compile_dir.unwrap().to_owned() + "/shader.spv")?)
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CompileRequest {
	network: graph_craft::proto::ProtoNetwork,
	input_types: Vec<Type>,
	output_type: Type,
}

impl CompileRequest {
	pub fn new(network: ProtoNetwork, input_types: Vec<Type>, output_type: Type) -> Self {
		Self { network, input_types, output_type }
	}
	pub fn compile(&self, compile_dir: &str, manifest_path: &str) -> anyhow::Result<Vec<u8>> {
		compile_spirv(self, Some(compile_dir), manifest_path)
	}
}
