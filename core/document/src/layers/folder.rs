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
		let pos = self.position_of_layer(id)?;
		self.layers.remove(pos);
		self.layer_ids.remove(pos);
		Ok(())
	}

	pub fn reorder_layers(&mut self, source_ids: Vec<LayerId>, target_id: LayerId) -> Result<(), DocumentError> {
		let source_pos = self.position_of_layer(source_ids[0])?;
		let source_pos_end = source_pos + source_ids.len() - 1;
		let target_pos = self.position_of_layer(target_id)?;

		let mut last_pos = source_pos;
		for layer_id in &source_ids[1..source_ids.len()] {
			let layer_pos = self.position_of_layer(*layer_id)?;
			if (layer_pos as i32 - last_pos as i32).abs() > 1 {
				// Selection is not contiguous
				return Err(DocumentError::NonReorderableSelection);
			}
			last_pos = layer_pos;
		}

		if source_pos < target_pos {
			// Moving layers up the hierarchy

			// Prevent shifting past end
			if source_pos_end + 1 >= self.layers.len() {
				return Err(DocumentError::NonReorderableSelection);
			}

			fn reorder_up<T>(arr: &mut Vec<T>, source_pos: usize, source_pos_end: usize, target_pos: usize)
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

			reorder_up(&mut self.layers, source_pos, source_pos_end, target_pos);
			reorder_up(&mut self.layer_ids, source_pos, source_pos_end, target_pos);
		} else {
			// Moving layers down the hierarchy

			// Prevent shifting past end
			if source_pos == 0 {
				return Err(DocumentError::NonReorderableSelection);
			}

			fn reorder_down<T>(arr: &mut Vec<T>, source_pos: usize, source_pos_end: usize, target_pos: usize)
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

			reorder_down(&mut self.layers, source_pos, source_pos_end, target_pos);
			reorder_down(&mut self.layer_ids, source_pos, source_pos_end, target_pos);
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

#[cfg(test)]
mod test {
	use glam::{DAffine2, DVec2};

	use crate::layers::{style::PathStyle, Ellipse, Layer, LayerDataTypes, Line, PolyLine, Rect, Shape};

	use super::Folder;

	#[test]
	fn reorder_layers() {
		let mut folder = Folder::default();

		let identity_transform = DAffine2::IDENTITY.to_cols_array();
		folder.add_layer(Layer::new(LayerDataTypes::Shape(Shape::new(true, 3)), identity_transform, PathStyle::default()), 0);
		folder.add_layer(Layer::new(LayerDataTypes::Rect(Rect::default()), identity_transform, PathStyle::default()), 1);
		folder.add_layer(Layer::new(LayerDataTypes::Ellipse(Ellipse::default()), identity_transform, PathStyle::default()), 2);
		folder.add_layer(Layer::new(LayerDataTypes::Line(Line::default()), identity_transform, PathStyle::default()), 3);
		folder.add_layer(
			Layer::new(LayerDataTypes::PolyLine(PolyLine::new(vec![DVec2::ZERO, DVec2::ONE])), identity_transform, PathStyle::default()),
			4,
		);

		assert_eq!(folder.layer_ids[0], 0);
		assert_eq!(folder.layer_ids[1], 1);
		assert_eq!(folder.layer_ids[2], 2);
		assert_eq!(folder.layer_ids[3], 3);
		assert_eq!(folder.layer_ids[4], 4);

		assert!(matches!(folder.layer(0).unwrap().data, LayerDataTypes::Shape(_)));
		assert!(matches!(folder.layer(1).unwrap().data, LayerDataTypes::Rect(_)));
		assert!(matches!(folder.layer(2).unwrap().data, LayerDataTypes::Ellipse(_)));
		assert!(matches!(folder.layer(3).unwrap().data, LayerDataTypes::Line(_)));
		assert!(matches!(folder.layer(4).unwrap().data, LayerDataTypes::PolyLine(_)));

		assert_eq!(folder.layer_ids.len(), 5);
		assert_eq!(folder.layers.len(), 5);

		folder.reorder_layers(vec![0, 1], 2).unwrap();

		assert_eq!(folder.layer_ids[0], 2);
		// Moved layers
		assert_eq!(folder.layer_ids[1], 0);
		assert_eq!(folder.layer_ids[2], 1);

		assert_eq!(folder.layer_ids[3], 3);
		assert_eq!(folder.layer_ids[4], 4);

		assert!(matches!(folder.layer(2).unwrap().data, LayerDataTypes::Ellipse(_)));
		// Moved layers
		assert!(matches!(folder.layer(0).unwrap().data, LayerDataTypes::Shape(_)));
		assert!(matches!(folder.layer(1).unwrap().data, LayerDataTypes::Rect(_)));

		assert!(matches!(folder.layer(3).unwrap().data, LayerDataTypes::Line(_)));
		assert!(matches!(folder.layer(4).unwrap().data, LayerDataTypes::PolyLine(_)));

		assert_eq!(folder.layer_ids.len(), 5);
		assert_eq!(folder.layers.len(), 5);
	}

	#[test]
	fn reorder_layer_to_top() {
		let mut folder = Folder::default();

		let identity_transform = DAffine2::IDENTITY.to_cols_array();
		folder.add_layer(Layer::new(LayerDataTypes::Shape(Shape::new(true, 3)), identity_transform, PathStyle::default()), 0);
		folder.add_layer(Layer::new(LayerDataTypes::Rect(Rect::default()), identity_transform, PathStyle::default()), 1);
		folder.add_layer(Layer::new(LayerDataTypes::Ellipse(Ellipse::default()), identity_transform, PathStyle::default()), 2);
		folder.add_layer(Layer::new(LayerDataTypes::Line(Line::default()), identity_transform, PathStyle::default()), 3);

		assert_eq!(folder.layer_ids[0], 0);
		assert_eq!(folder.layer_ids[1], 1);
		assert_eq!(folder.layer_ids[2], 2);
		assert_eq!(folder.layer_ids[3], 3);

		assert!(matches!(folder.layer(0).unwrap().data, LayerDataTypes::Shape(_)));
		assert!(matches!(folder.layer(1).unwrap().data, LayerDataTypes::Rect(_)));
		assert!(matches!(folder.layer(2).unwrap().data, LayerDataTypes::Ellipse(_)));
		assert!(matches!(folder.layer(3).unwrap().data, LayerDataTypes::Line(_)));

		assert_eq!(folder.layer_ids.len(), 4);
		assert_eq!(folder.layers.len(), 4);

		folder.reorder_layers(vec![1], 3).unwrap();

		assert_eq!(folder.layer_ids[0], 0);
		assert_eq!(folder.layer_ids[1], 2);
		assert_eq!(folder.layer_ids[2], 3);
		// Moved layer
		assert_eq!(folder.layer_ids[3], 1);

		assert!(matches!(folder.layer(0).unwrap().data, LayerDataTypes::Shape(_)));
		assert!(matches!(folder.layer(2).unwrap().data, LayerDataTypes::Ellipse(_)));
		assert!(matches!(folder.layer(3).unwrap().data, LayerDataTypes::Line(_)));
		// Moved layer
		assert!(matches!(folder.layer(1).unwrap().data, LayerDataTypes::Rect(_)));

		assert_eq!(folder.layer_ids.len(), 4);
		assert_eq!(folder.layers.len(), 4);
	}

	#[test]
	fn reorder_non_contiguous_selection() {
		let mut folder = Folder::default();

		let identity_transform = DAffine2::IDENTITY.to_cols_array();
		folder.add_layer(Layer::new(LayerDataTypes::Shape(Shape::new(true, 3)), identity_transform, PathStyle::default()), 0);
		folder.add_layer(Layer::new(LayerDataTypes::Rect(Rect::default()), identity_transform, PathStyle::default()), 1);
		folder.add_layer(Layer::new(LayerDataTypes::Ellipse(Ellipse::default()), identity_transform, PathStyle::default()), 2);
		folder.add_layer(Layer::new(LayerDataTypes::Line(Line::default()), identity_transform, PathStyle::default()), 3);

		assert_eq!(folder.layer_ids[0], 0);
		assert_eq!(folder.layer_ids[1], 1);
		assert_eq!(folder.layer_ids[2], 2);
		assert_eq!(folder.layer_ids[3], 3);

		assert!(matches!(folder.layer(0).unwrap().data, LayerDataTypes::Shape(_)));
		assert!(matches!(folder.layer(1).unwrap().data, LayerDataTypes::Rect(_)));
		assert!(matches!(folder.layer(2).unwrap().data, LayerDataTypes::Ellipse(_)));
		assert!(matches!(folder.layer(3).unwrap().data, LayerDataTypes::Line(_)));

		assert_eq!(folder.layer_ids.len(), 4);
		assert_eq!(folder.layers.len(), 4);

		folder.reorder_layers(vec![0, 2], 3).expect_err("Non-contiguous selections can't be reordered");

		// Expect identical state
		assert_eq!(folder.layer_ids[0], 0);
		assert_eq!(folder.layer_ids[1], 1);
		assert_eq!(folder.layer_ids[2], 2);
		assert_eq!(folder.layer_ids[3], 3);

		assert!(matches!(folder.layer(0).unwrap().data, LayerDataTypes::Shape(_)));
		assert!(matches!(folder.layer(1).unwrap().data, LayerDataTypes::Rect(_)));
		assert!(matches!(folder.layer(2).unwrap().data, LayerDataTypes::Ellipse(_)));
		assert!(matches!(folder.layer(3).unwrap().data, LayerDataTypes::Line(_)));

		assert_eq!(folder.layer_ids.len(), 4);
		assert_eq!(folder.layers.len(), 4);
	}
}
