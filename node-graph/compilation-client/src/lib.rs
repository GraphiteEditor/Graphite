use gpu_compiler_bin_wrapper::CompileRequest;
use gpu_executor::ShaderIO;
use graph_craft::{proto::ProtoNetwork, Type};

pub async fn compile(network: ProtoNetwork, inputs: Vec<Type>, output: Type, io: ShaderIO) -> Result<Shader, reqwest::Error> {
	let client = reqwest::Client::new();

	let compile_request = CompileRequest::new(network, inputs.clone(), output.clone(), io.clone());
	let response = client.post("http://localhost:3000/compile/spirv").json(&compile_request).send();
	let response = response.await?;
	response.bytes().await.map(|b| Shader {
		bytes: b.windows(4).map(|x| u32::from_le_bytes(x.try_into().unwrap())).collect(),
		input_types: inputs,
		output_type: output,
		io,
	})
}

pub fn compile_sync(network: ProtoNetwork, inputs: Vec<Type>, output: Type, io: ShaderIO) -> Result<Shader, reqwest::Error> {
	future_executor::block_on(compile(network, inputs, output, io))
}

// TODO: should we add the entry point as a field?
/// A compiled shader with type annotations.
pub struct Shader {
	pub bytes: Vec<u32>,
	pub input_types: Vec<Type>,
	pub output_type: Type,
	pub io: ShaderIO,
}
