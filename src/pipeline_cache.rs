use std::collections::HashMap;
use super::pipeline::Pipeline;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PipelineID {
	index: usize,
}

impl PipelineID {
	pub fn new(index: usize) -> Self {
		Self { index }
	}
}

pub struct PipelineCache {
	pub pipelines: Vec<Pipeline>,
	pub name_to_id: HashMap<String, PipelineID>,
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

	pub fn get_by_name(&self, name: &str) -> Option<&Pipeline> {
		match self.name_to_id.get(name) {
			Some(id) => self.pipelines.get(id.index),
			None => None,
		}
	}

	pub fn get_by_id(&self, id: PipelineID) -> Option<&Pipeline> {
		self.pipelines.get(id.index)
	}

	pub fn set(&mut self, name: &str, pipeline: Pipeline) -> PipelineID {
		match self.name_to_id.get(name) {
			Some(id) => {
				self.pipelines[id.index] = pipeline;
				id.clone()
			},
			None => {
				let last_index = self.name_to_id.len();
				let id = PipelineID::new(last_index);
				self.name_to_id.insert(String::from(name), id);
				self.pipelines.push(pipeline);
				id
			}
		}
	}
}