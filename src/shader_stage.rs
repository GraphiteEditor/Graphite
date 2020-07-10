use std::fs;
use std::io;

pub fn compile_from_glsl(device: &wgpu::Device, path: &str, shader_type: glsl_to_spirv::ShaderType) -> io::Result<wgpu::ShaderModule> {
	let source = fs::read_to_string(path)?;
	let spirv = match glsl_to_spirv::compile(&source[..], shader_type) {
		Ok(spirv_output) => spirv_output,
		Err(message) => {
			println!("Error compiling GLSL to SPIRV shader: {}", message);
			panic!("{}", message);
		},
	};
	let compiled = wgpu::read_spirv(spirv)?;
	let shader = device.create_shader_module(&compiled);

	Ok(shader)
}