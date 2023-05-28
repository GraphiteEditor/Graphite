use gpu_executor::ShaderIO;
use graph_craft::{proto::ProtoNetwork, Type};

use serde::{Deserialize, Serialize};
use std::io::Write;

pub fn compile_spirv(request: &CompileRequest, compile_dir: Option<&str>, manifest_path: &str) -> anyhow::Result<Vec<u8>> {
	let serialized_graph = serde_json::to_string(&gpu_executor::CompileRequest {
		networks: request.networks.clone(),
		io: request.shader_io.clone(),
	})?;

	#[cfg(not(feature = "profiling"))]
	let features = "";
	#[cfg(feature = "profiling")]
	let features = "profiling";

	println!("calling cargo run!");
	let non_cargo_env_vars = std::env::vars().filter(|(k, _)| k.starts_with("PATH")).collect::<Vec<_>>();
	let mut cargo_command = std::process::Command::new("cargo")
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq)]
pub struct CompileRequest {
	networks: Vec<graph_craft::proto::ProtoNetwork>,
	input_types: Vec<Type>,
	output_types: Vec<Type>,
	shader_io: ShaderIO,
}

impl CompileRequest {
	pub fn new(networks: Vec<ProtoNetwork>, input_types: Vec<Type>, output_types: Vec<Type>, io: ShaderIO) -> Self {
		// TODO: add type checking
		// for (input, buffer) in input_types.iter().zip(io.inputs.iter()) {
		// 	assert_eq!(input, &buffer.ty());
		// }
		// assert_eq!(output_type, io.output.ty());
		Self {
			networks,
			input_types,
			output_types,
			shader_io: io,
		}
	}
	pub fn compile(&self, compile_dir: &str, manifest_path: &str) -> anyhow::Result<Vec<u8>> {
		compile_spirv(self, Some(compile_dir), manifest_path)
	}
}
