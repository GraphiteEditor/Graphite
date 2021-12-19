use glam::DVec2;

use crate::{DocumentError, LayerId, Quad};

use super::{Layer, LayerData, LayerDataType};

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct Folder {
	next_assignment_id: LayerId,
	pub layer_ids: Vec<LayerId>,
	layers: Vec<Layer>,
}

impl LayerData for Folder {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<glam::DAffine2>) {
		for layer in &mut self.layers {
			let _ = writeln!(svg, "{}", layer.render(transforms));
		}
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		for (layer, layer_id) in self.layers().iter().zip(&self.layer_ids) {
			path.push(*layer_id);
			layer.intersects_quad(quad, path, intersections);
			path.pop();
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		self.layers
			.iter()
			.filter_map(|layer| layer.data.bounding_box(transform * layer.transform))
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
	}
}

impl Folder {
	/// When a insertion id is provided, try to insert the layer with the given id.
	/// If that id is already used, return None.
	/// When no insertion id is provided, search for the next free id and insert it with that.
	/// Negative values for insert_index represent distance from the end
	pub fn add_layer(&mut self, layer: Layer, id: Option<LayerId>, insert_index: isize) -> Option<LayerId> {
		let mut insert_index = insert_index as i128;
		if insert_index < 0 {
			insert_index = self.layers.len() as i128 + insert_index as i128 + 1;
		}

		if insert_index <= self.layers.len() as i128 && insert_index >= 0 {
			if let Some(id) = id {
				self.next_assignment_id = id;
			}
			if self.layer_ids.contains(&self.next_assignment_id) {
				return None;
			}

			let id = self.next_assignment_id;
			self.layers.insert(insert_index as usize, layer);
			self.layer_ids.insert(insert_index as usize, id);

			// Linear probing for collision avoidance
			while self.layer_ids.contains(&self.next_assignment_id) {
				self.next_assignment_id += 1;
			}

			Some(id)
		} else {
			None
		}
	}

	pub fn remove_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
		let pos = self.position_of_layer(id)?;
		self.layers.remove(pos);
		self.layer_ids.remove(pos);
		Ok(())
	}

	/// Returns a list of layers in the folder
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	pub fn layers(&self) -> &[Layer] {
		self.layers.as_slice()
	}

	pub fn layers_mut(&mut self) -> &mut [Layer] {
		self.layers.as_mut_slice()
	}

	pub fn layer(&self, id: LayerId) -> Option<&Layer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&self.layers[pos])
	}

	pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&mut self.layers[pos])
	}

	pub fn position_of_layer(&self, layer_id: LayerId) -> Result<usize, DocumentError> {
		self.layer_ids.iter().position(|x| *x == layer_id).ok_or(DocumentError::LayerNotFound)
	}

	pub fn folder(&self, id: LayerId) -> Option<&Folder> {
		match self.layer(id) {
			Some(Layer {
				data: LayerDataType::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}

	pub fn folder_mut(&mut self, id: LayerId) -> Option<&mut Folder> {
		match self.layer_mut(id) {
			Some(Layer {
				data: LayerDataType::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}
}
