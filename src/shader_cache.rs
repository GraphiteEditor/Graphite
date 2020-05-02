use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ShaderID {
	index: usize,
}

impl ShaderID {
	pub fn new(index: usize) -> Self {
		Self { index }
	}
}

pub struct ShaderCache {
	pub shaders: Vec<wgpu::ShaderModule>,
	pub path_to_id: HashMap<String, ShaderID>,
}

impl ShaderCache {
	pub fn new() -> Self {
		let shaders = Vec::new();
		let path_to_id = HashMap::new();

		Self {
			shaders,
			path_to_id,
		}
	}

	pub fn get_by_path(&self, path: &str) -> Option<&wgpu::ShaderModule> {
		match self.path_to_id.get(path) {
			Some(id) => self.shaders.get(id.index),
			None => None,
		}
	}

	// pub fn get_by_id(&self, id: ShaderID) -> Option<&wgpu::ShaderModule> {
	// 	self.shaders.get(id.index)
	// }

	pub fn load(&mut self, device: &wgpu::Device, path: &str, shader_type: glsl_to_spirv::ShaderType) -> Result<(), std::io::Error> {
		if self.path_to_id.get(path).is_none() {
			let source = std::fs::read_to_string(path)?;
			let spirv = glsl_to_spirv::compile(&source[..], shader_type).unwrap();
			let compiled = wgpu::read_spirv(spirv).unwrap();
			let shader = device.create_shader_module(&compiled);

			let last_index = self.path_to_id.len();
			self.path_to_id.insert(String::from(path), ShaderID { index: last_index });
			self.shaders.push(shader);
		}

		Ok(())
	}
}