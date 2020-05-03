use std::collections::HashMap;
use super::pipeline::Pipeline;

#[derive(Copy, Clone, PartialEq, Debug)]
struct CacheID {
	index: usize,
}

impl CacheID {
	fn new(index: usize) -> Self {
		Self { index }
	}
}

pub struct PipelineCache {
	pub pipelines: Vec<Pipeline>,
	name_to_id: HashMap<String, CacheID>,
}

impl PipelineCache {
	pub fn new() -> Self {
		let pipelines = Vec::new();
		let name_to_id = HashMap::new();

		Self {
			pipelines,
			name_to_id,
		}
	}

	#[allow(dead_code)]
	pub fn get(&self, name: &str) -> Option<&Pipeline> {
		match self.name_to_id.get(name) {
			Some(id) => self.pipelines.get(id.index),
			None => None,
		}
	}

	#[allow(dead_code)]
	pub fn set(&mut self, name: &str, pipeline: Pipeline) {
		match self.name_to_id.get(name) {
			Some(id) => {
				self.pipelines[id.index] = pipeline;
			},
			None => {
				let last_index = self.name_to_id.len();
				let id = CacheID::new(last_index);
				self.name_to_id.insert(String::from(name), id);
				self.pipelines.push(pipeline);
			}
		}
	}

	#[allow(dead_code)]
	pub fn load(&mut self, device: &wgpu::Device, name: &str, vertex_shader: &wgpu::ShaderModule, fragment_shader: &wgpu::ShaderModule) {
		let pipeline = Pipeline::new(device, vertex_shader, fragment_shader);
		self.set(name, pipeline);
	}
}