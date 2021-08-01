use glam::DVec2;

use crate::{DocumentError, LayerId};

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

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		for (layer, layer_id) in self.layers().iter().zip(&self.layer_ids) {
			path.push(*layer_id);
			layer.intersects_quad(quad, path, intersections);
			path.pop();
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut layers_non_empty_bounding_boxes = self.layers.iter().filter_map(|layer| layer.data.bounding_box(transform * layer.transform)).peekable();

		layers_non_empty_bounding_boxes.peek()?;

		let mut x_min = f64::MAX;
		let mut y_min = f64::MAX;
		let mut x_max = f64::MIN;
		let mut y_max = f64::MIN;

		for [bounding_box_min, bounding_box_max] in layers_non_empty_bounding_boxes {
			if bounding_box_min.x < x_min {
				x_min = bounding_box_min.x
			}
			if bounding_box_min.y < y_min {
				y_min = bounding_box_min.y
			}
			if bounding_box_max.x > x_max {
				x_max = bounding_box_max.x
			}
			if bounding_box_max.y > y_max {
				y_max = bounding_box_max.y
			}
		}
		Some([DVec2::new(x_min, y_min), DVec2::new(x_max, y_max)])
	}
}

impl Folder {
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
