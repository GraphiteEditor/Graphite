use gpu_compiler_bin_wrapper::CompileRequest;
use gpu_executor::ShaderIO;
use graph_craft::{proto::ProtoNetwork, Type};

pub async fn compile(networks: Vec<ProtoNetwork>, inputs: Vec<Type>, outputs: Vec<Type>, io: ShaderIO) -> Result<Shader, reqwest::Error> {
	let client = reqwest::Client::new();

	let compile_request = CompileRequest::new(networks, inputs.clone(), outputs.clone(), io.clone());
	let response = client.post("http://localhost:3000/compile/spirv").json(&compile_request).send();
	let response = response.await?;
	response.bytes().await.map(|b| Shader {
		spirv_binary: b.chunks(4).map(|x| u32::from_le_bytes(x.try_into().unwrap())).collect(),
		input_types: inputs,
		output_types: outputs,
		io,
	})
}

// TODO: should we add the entry point as a field?
/// A compiled shader with type annotations.
pub struct Shader {
	pub spirv_binary: Vec<u32>,
	pub input_types: Vec<Type>,
	pub output_types: Vec<Type>,
	pub io: ShaderIO,
}
