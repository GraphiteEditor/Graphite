use crate::{DocumentError, LayerId};

use super::{Layer, LayerData, LayerDataTypes};

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
	next_assignment_id: LayerId,
	layer_ids: Vec<LayerId>,
	layers: Vec<Layer>,
}

impl LayerData for Folder {
	fn render(&self) -> String {
		self.layers
			.iter()
			.filter(|layer| layer.visible)
			.map(|layer| layer.data.render())
			.fold(String::with_capacity(self.layers.len() * 30), |s, n| s + "\n" + &n)
	}
}
impl Folder {
	pub fn add_layer(&mut self, layer: Layer, insert_index: isize) -> Option<LayerId> {
		let mut insert_index = insert_index as i128;
		if insert_index < 0 {
			insert_index = self.layers.len() as i128 + insert_index as i128 + 1;
		}

		if insert_index <= self.layers.len() as i128 && insert_index >= 0 {
			self.layers.insert(insert_index as usize, layer);
			self.layer_ids.insert(insert_index as usize, self.next_assignment_id);
			self.next_assignment_id += 1;
			Some(self.next_assignment_id - 1)
		} else {
			None
		}
	}

	pub fn remove_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
		let pos = self.layer_ids.iter().position(|x| *x == id).ok_or(DocumentError::LayerNotFound)?;
		self.layers.remove(pos);
		self.layer_ids.remove(pos);
		Ok(())
	}

	/// Returns a list of layers in the folder
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	pub fn layer(&self, id: LayerId) -> Option<&Layer> {
		let pos = self.layer_ids.iter().position(|x| *x == id)?;
		Some(&self.layers[pos])
	}

	pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
		let pos = self.layer_ids.iter().position(|x| *x == id)?;
		Some(&mut self.layers[pos])
	}

	pub fn folder(&self, id: LayerId) -> Option<&Folder> {
		match self.layer(id) {
			Some(Layer {
				data: LayerDataTypes::Folder(folder), ..
			}) => Some(&folder),
			_ => None,
		}
	}

	pub fn folder_mut(&mut self, id: LayerId) -> Option<&mut Folder> {
		match self.layer_mut(id) {
			Some(Layer {
				data: LayerDataTypes::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}
}

impl Default for Folder {
	fn default() -> Self {
		Self {
			layer_ids: vec![],
			layers: vec![],
			next_assignment_id: 0,
		}
	}
}
