use layers::PolyLine;

use crate::{
	layers::{self, Folder, Layer, LayerData, LayerDataTypes, Line, Rect, Shape},
	DocumentError, LayerId, Operation,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
	pub root: layers::Folder,
	pub work: Folder,
	pub work_mount_path: Vec<LayerId>,
	pub work_operations: Vec<Operation>,
	pub work_mounted: bool,
}

impl Default for Document {
	fn default() -> Self {
		Self {
			root: Folder::default(),
			work: Folder::default(),
			work_mount_path: Vec::new(),
			work_operations: Vec::new(),
			work_mounted: false,
		}
	}
}

fn split_path(path: &[LayerId]) -> Result<(&[LayerId], LayerId), DocumentError> {
	let id = path.last().ok_or(DocumentError::InvalidPath)?;
	let folder_path = &path[0..path.len() - 1];
	Ok((folder_path, *id))
}

impl Document {
	pub fn render(&self, path: &mut Vec<LayerId>) -> String {
		if !self.work_mount_path.as_slice().starts_with(path) {
			match &self.layer(path).unwrap().data {
				LayerDataTypes::Folder(_) => (),
				element => {
					path.pop();
					return element.render();
				}
			}
		}
		if path.as_slice() == self.work_mount_path {
			let mut out = self.document_folder(path).unwrap().render();
			out += self.work.render().as_str();
			path.pop();
			return out;
		}
		let mut out = String::with_capacity(30);
		for element in self.folder(path).unwrap().layer_ids.iter() {
			path.push(*element);
			out += self.render(path).as_str();
		}
		out
	}

	fn is_mounted(&self, mount_path: &[LayerId], path: &[LayerId]) -> bool {
		path.starts_with(mount_path) && self.work_mounted
	}

	pub fn folder(&self, mut path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		if self.is_mounted(self.work_mount_path.as_slice(), path) {
			path = &path[self.work_mount_path.len()..];
			root = &self.work;
		}
		for id in path {
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn folder_mut(&mut self, mut path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
		let mut root = if self.is_mounted(self.work_mount_path.as_slice(), path) {
			path = &path[self.work_mount_path.len()..];
			&mut self.work
		} else {
			&mut self.root
		};
		for id in path {
			root = root.folder_mut(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn document_folder(&self, path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn document_folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
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
		self.document_folder_mut(path)?.remove_layer(id)?;
		Ok(())
	}

	pub fn handle_operation<F: Fn(String)>(&mut self, operation: Operation, update_frontend: &F) -> Result<(), DocumentError> {
		self.work_operations.push(operation.clone());
		match operation {
			Operation::AddCircle { path, insert_index, cx, cy, r, style } => {
				self.add_layer(&path, Layer::new(LayerDataTypes::Circle(layers::Circle::new(kurbo::Point::new(cx, cy), r, style))), insert_index)?;

				update_frontend(self.render(&mut vec![]));
			}
			Operation::AddRect {
				path,
				insert_index,
				x0,
				y0,
				x1,
				y1,
				style,
			} => {
				self.add_layer(
					&path,
					Layer::new(LayerDataTypes::Rect(Rect::new(kurbo::Point::new(x0, y0), kurbo::Point::new(x1, y1), style))),
					insert_index,
				)?;

				update_frontend(self.render(&mut vec![]));
			}
			Operation::AddLine {
				path,
				insert_index,
				x0,
				y0,
				x1,
				y1,
				style,
			} => {
				self.add_layer(
					&path,
					Layer::new(LayerDataTypes::Line(Line::new(kurbo::Point::new(x0, y0), kurbo::Point::new(x1, y1), style))),
					insert_index,
				)?;

				update_frontend(self.render(&mut vec![]));
			}
			Operation::AddPen { path, insert_index, points, style } => {
				let points: Vec<kurbo::Point> = points.into_iter().map(|it| it.into()).collect();
				let pl = PolyLine::new(points, style);
				self.add_layer(&path, Layer::new(LayerDataTypes::PolyLine(pl)), insert_index)?;
				update_frontend(self.render(&mut vec![]));
			}
			Operation::AddShape {
				path,
				insert_index,
				x0,
				y0,
				x1,
				y1,
				sides,
				style,
			} => {
				let s = Shape::new(kurbo::Point::new(x0, y0), kurbo::Vec2 { x: x0 - x1, y: y0 - y1 }, sides, style);
				self.add_layer(&path, Layer::new(LayerDataTypes::Shape(s)), insert_index)?;

				update_frontend(self.render(&mut vec![]));
			}
			Operation::DeleteLayer { path } => {
				self.delete(&path)?;

				update_frontend(self.render(&mut vec![]));
			}
			Operation::AddFolder { path } => self.set_layer(&path, Layer::new(LayerDataTypes::Folder(Folder::default())))?,
			Operation::MountWorkingFolder { path } => {
				self.work_operations.clear();
				self.work_mount_path = path;
				self.work = Folder::default();
				self.work_mounted = true;
			}
			Operation::DiscardWorkingFolder => {
				self.work_operations.clear();
				self.work_mount_path = vec![];
				self.work = Folder::default();
				self.work_mounted = false;
			}
			Operation::ClearWorkingFolder => {
				self.work_operations.clear();
				self.work = Folder::default();
			}
			Operation::CommitTransaction => {
				let mut ops = Vec::new();
				std::mem::swap(&mut ops, &mut self.work_operations);
				let len = ops.len() - 1;
				self.work_mounted = false;
				self.work_mount_path = vec![];
				self.work = Folder::default();
				for operation in ops.into_iter().take(len) {
					self.handle_operation(operation, update_frontend)?
				}

				update_frontend(self.render(&mut vec![]));
			}
		}
		Ok(())
	}
}
