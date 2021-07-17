use glam::DVec2;

use crate::{DocumentError, LayerId};

use super::{style, Layer, LayerData, LayerDataTypes};

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Folder {
	next_assignment_id: LayerId,
	pub layer_ids: Vec<LayerId>,
	layers: Vec<Layer>,
}

impl LayerData for Folder {
	fn to_kurbo_path(&self, _: glam::DAffine2, _: style::PathStyle) -> kurbo::BezPath {
		unimplemented!()
	}

	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, _style: style::PathStyle) {
		let _ = writeln!(svg, r#"<g transform="matrix("#);
		transform.to_cols_array().iter().enumerate().for_each(|(i, f)| {
			let _ = svg.write_str(&(f.to_string() + if i != 5 { "," } else { "" }));
		});
		let _ = svg.write_str(r#")">"#);

		for layer in &mut self.layers {
			let _ = writeln!(svg, "{}", layer.render());
		}
		let _ = writeln!(svg, "</g>");
	}

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, _style: style::PathStyle) {
		for (layer, layer_id) in self.layers().iter().zip(&self.layer_ids) {
			path.push(*layer_id);
			layer.intersects_quad(quad, path, intersections);
			path.pop();
		}
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

	pub fn reorder_layers(&mut self, source_ids: Vec<LayerId>, target_id: LayerId) -> Result<(), DocumentError> {
		let source_pos = self.layer_ids.iter().position(|x| *x == source_ids[0]).ok_or(DocumentError::LayerNotFound)?;
		let source_pos_end = source_pos + source_ids.len() - 1;
		let target_pos = self.layer_ids.iter().position(|x| *x == target_id).ok_or(DocumentError::LayerNotFound)?;

		let mut last_pos = source_pos;
		for layer_id in &source_ids[1..source_ids.len()] {
			let layer_pos = self.layer_ids.iter().position(|x| *x == *layer_id).ok_or(DocumentError::LayerNotFound)?;
			if (layer_pos as i32 - last_pos as i32).abs() > 1 {
				// Selection is not contiguous
				return Err(DocumentError::InvalidPath);
			}
			last_pos = layer_pos;
		}

		if source_pos < target_pos {
			// Dragging up

			// Prevent shifting past end
			if source_pos_end + 1 >= self.layers.len() {
				return Err(DocumentError::InvalidPath);
			}

			fn rearrange<T>(arr: &mut Vec<T>, source_pos: usize, source_pos_end: usize, target_pos: usize)
			where
				T: Clone,
			{
				*arr = [
					&arr[0..source_pos],                   // Elements before selection
					&arr[source_pos_end + 1..=target_pos], // Elements between selection end and target
					&arr[source_pos..=source_pos_end],     // Selection itself
					&arr[target_pos + 1..],                // Elements before target
				]
				.concat();
			}

			rearrange(&mut self.layers, source_pos, source_pos_end, target_pos);
			rearrange(&mut self.layer_ids, source_pos, source_pos_end, target_pos);

			let min_index = source_pos_end.min(target_pos);
			let max_index = source_pos_end.max(target_pos);
			for layer_index in min_index..max_index {
				self.layers[layer_index].cache_dirty = true;
			}
		} else {
			// Dragging down

			// Prevent shifting past end
			if source_pos == 0 {
				return Err(DocumentError::InvalidPath);
			}

			fn rearrange<T>(arr: &mut Vec<T>, source_pos: usize, source_pos_end: usize, target_pos: usize)
			where
				T: Clone,
			{
				*arr = [
					&arr[0..target_pos],               // Elements before target
					&arr[source_pos..=source_pos_end], // Selection itself
					&arr[target_pos..source_pos],      // Elements between selection and target
					&arr[source_pos_end + 1..],        // Elements before selection
				]
				.concat();
			}

			rearrange(&mut self.layers, source_pos, source_pos_end, target_pos);
			rearrange(&mut self.layer_ids, source_pos, source_pos_end, target_pos);

			let min_index = source_pos.min(target_pos);
			let max_index = source_pos.max(target_pos);
			for layer_index in min_index..max_index {
				self.layers[layer_index].cache_dirty = true;
			}
		}

		Ok(())
	}

	/// Returns a list of layers in the folder
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	pub fn layers(&self) -> &[Layer] {
		self.layers.as_slice()
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

	pub fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut layers_non_empty_bounding_boxes = self.layers.iter().filter_map(|layer| layer.bounding_box(transform * layer.transform, layer.style)).peekable();

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

impl Default for Folder {
	fn default() -> Self {
		Self {
			layer_ids: vec![],
			layers: vec![],
			next_assignment_id: 0,
		}
	}
}
