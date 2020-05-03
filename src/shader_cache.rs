use std::collections::HashMap;


#[derive(Copy, Clone, PartialEq, Debug)]
struct CacheID {
	index: usize,
}

impl CacheID {
	fn new(index: usize) -> Self {
		Self { index }
	}
}

pub struct ShaderCache {
	pub shaders: Vec<wgpu::ShaderModule>,
	name_to_id: HashMap<String, CacheID>,
}

impl ShaderCache {
	pub fn new() -> Self {
		let shaders = Vec::new();
		let name_to_id = HashMap::new();

		Self {
			shaders,
			name_to_id,
		}
	}

	#[allow(dead_code)]
	pub fn get(&self, name: &str) -> Option<&wgpu::ShaderModule> {
		match self.name_to_id.get(name) {
			Some(id) => self.shaders.get(id.index),
			None => None,
		}
	}

	#[allow(dead_code)]
	pub fn set(&mut self, name: &str, shader: wgpu::ShaderModule) {
		match self.name_to_id.get(name) {
			Some(id) => {
				self.shaders[id.index] = shader;
			},
			None => {
				let last_index = self.name_to_id.len();
				let id = CacheID::new(last_index);
				self.name_to_id.insert(String::from(name), id);
				self.shaders.push(shader);
			}
		}
	}

	#[allow(dead_code)]
	pub fn load(&mut self, device: &wgpu::Device, path: &str, shader_type: glsl_to_spirv::ShaderType) -> std::io::Result<()> {
		if self.name_to_id.get(path).is_none() {
			let source = std::fs::read_to_string(path)?;
			let spirv = match glsl_to_spirv::compile(&source[..], shader_type) {
				Ok(spirv_output) => spirv_output,
				Err(message) => {
					println!("Error compiling GLSL to SPIRV shader: {}", message);
					panic!("{}", message);
				}
			};
			let compiled = wgpu::read_spirv(spirv)?;
			let shader = device.create_shader_module(&compiled);

			let last_index = self.name_to_id.len();
			self.name_to_id.insert(String::from(path), CacheID::new(last_index));
			self.shaders.push(shader);
		}

		Ok(())
	}
}