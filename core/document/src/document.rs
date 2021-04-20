use crate::{
	layers::{self, Folder, Layer, LayerData, LayerDataTypes, Line, Rect, Shape},
	DocumentError, LayerId, Operation,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
	pub root: layers::Folder,
}

impl Default for Document {
	fn default() -> Self {
		Self { root: layers::Folder::default() }
	}
}

fn split_path(path: &[LayerId]) -> Result<(&[LayerId], LayerId), DocumentError> {
	let id = path.last().ok_or(DocumentError::InvalidPath)?;
	let folder_path = &path[0..path.len() - 1];
	Ok((folder_path, *id))
}

impl Document {
	pub fn render(&self) -> String {
		self.root.render()
	}

	pub fn folder(&self, path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.folder_mut(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn layer(&self, path: &[LayerId]) -> Result<&Layer, DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder(path)?.layer(id).ok_or(DocumentError::LayerNotFound)
	}

	pub fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or(DocumentError::LayerNotFound)
	}

	pub fn set_layer(&mut self, path: &[LayerId], layer: Layer) -> Result<(), DocumentError> {
		let mut folder = &mut self.root;
		if let Ok((path, id)) = split_path(path) {
			folder = self.folder_mut(path)?;
			if let Some(folder_layer) = folder.layer_mut(id) {
				*folder_layer = layer;
				return Ok(());
			}
		}
		folder.add_layer(layer, -1).ok_or(DocumentError::IndexOutOfBounds)?;
		Ok(())
	}

	/// Passing a negative `insert_index` indexes relative to the end
	/// -1 is equivalent to adding the layer to the top
	pub fn add_layer(&mut self, path: &[LayerId], layer: Layer, insert_index: isize) -> Result<LayerId, DocumentError> {
		let folder = self.folder_mut(path)?;
		folder.add_layer(layer, insert_index).ok_or(DocumentError::IndexOutOfBounds)
	}

	pub fn delete(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.remove_layer(id)?;
		Ok(())
	}

	pub fn handle_operation<F: Fn(String)>(&mut self, operation: Operation, update_frontend: F) -> Result<(), DocumentError> {
		match operation {
			Operation::AddCircle {
				path,
				insert_index,
				cx,
				cy,
				r,
				stroke,
				fill,
			} => {
				self.add_layer(&path, Layer::new(LayerDataTypes::Circle(layers::Circle::new(kurbo::Point::new(cx, cy), r, stroke, fill))), insert_index)?;

				update_frontend(self.render());
			}
			Operation::AddRect {
				path,
				insert_index,
				x0,
				y0,
				x1,
				y1,
				stroke,
				fill,
			} => {
				self.add_layer(
					&path,
					Layer::new(LayerDataTypes::Rect(Rect::new(kurbo::Point::new(x0, y0), kurbo::Point::new(x1, y1), stroke, fill))),
					insert_index,
				)?;

				update_frontend(self.render());
			}
			Operation::AddLine {
				path,
				insert_index,
				x0,
				y0,
				x1,
				y1,
				stroke,
			} => {
				self.add_layer(
					&path,
					Layer::new(LayerDataTypes::Line(Line::new(kurbo::Point::new(x0, y0), kurbo::Point::new(x1, y1), stroke))),
					insert_index,
				)?;

				update_frontend(self.render());
			}
			Operation::AddShape {
				path,
				insert_index,
				x0,
				y0,
				x1,
				y1,
				sides,
				stroke,
				fill,
			} => {
				let s = Shape::new(kurbo::Point::new(x0, y0), kurbo::Vec2 { x: x0 - x1, y: y0 - y1 }, sides, stroke, fill);
				self.add_layer(&path, Layer::new(LayerDataTypes::Shape(s)), insert_index)?;

				update_frontend(self.render());
			}
			Operation::DeleteLayer { path } => {
				self.delete(&path)?;

				update_frontend(self.render());
			}
			Operation::AddFolder { path } => self.set_layer(&path, Layer::new(LayerDataTypes::Folder(Folder::default())))?,
		}
		Ok(())
	}
}
