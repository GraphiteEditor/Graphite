use std::collections::HashMap;
use std::collections::HashMap;

#[derive(Copy, Clone)]
pub struct TextureID {
	pub index: usize,
}

pub struct TextureCache {
	pub textures: Vec<wgpu::Texture>,
	pub name_to_id: HashMap<String, TextureID>,
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

	pub fn get_by_path(&self, path: &str) -> Option<&wgpu::ShaderModule> {
		match self.name_to_id.get(path) {
			Some(id) => self.shaders.get(id.index),
			None => None,
		}
	}

	pub fn get_by_id(&self, id: ShaderID) -> Option<&wgpu::ShaderModule> {
		self.shaders.get(id.index)
	}

	pub fn load(&mut self, device: &wgpu::Device, path: &str, shader_type: glsl_to_spirv::ShaderType) -> std::io::Result<()> {
		if self.name_to_id.get(path).is_none() {
			let source = std::fs::read_to_string(path)?;
			let spirv = glsl_to_spirv::compile(&source[..], shader_type).unwrap();
			let compiled = wgpu::read_spirv(spirv).unwrap();
			let shader = device.create_shader_module(&compiled);

			let length = self.name_to_id.len();
			self.name_to_id.insert(String::from(path), ShaderID { index: length });
			self.shaders.push(shader);
		}

		Ok(())
	}
}