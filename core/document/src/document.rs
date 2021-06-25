use glam::DAffine2;

use crate::{
	layers::{self, style::PathStyle, Folder, Layer, LayerDataTypes, Line, PolyLine, Rect, Shape},
	DocumentError, DocumentResponse, LayerId, Operation,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
	pub root: Layer,
	pub work: Layer,
	pub work_mount_path: Vec<LayerId>,
	pub work_operations: Vec<Operation>,
	pub work_mounted: bool,
}

impl Default for Document {
	fn default() -> Self {
		Self {
			root: Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default()),
			work: Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default()),
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
	/// Renders the layer graph with the root `path` as an SVG string.
	/// This operation merges currently mounted folder and document_folder
	/// contents together.
	pub fn render(&mut self, path: &mut Vec<LayerId>, svg: &mut String) {
		if !self.work_mount_path.as_slice().starts_with(path) {
			self.layer_mut(path).unwrap().render();
			path.pop();
			return;
		}
		if path.as_slice() == self.work_mount_path {
			// TODO: Handle if mounted in nested folders
			let transform = self.document_folder(path).unwrap().transform.clone();
			self.document_folder_mut(path).unwrap().render_as_folder(svg);
			self.work.transform = transform;
			self.work.render_as_folder(svg);
			path.pop();
		}
		let ids = self.folder(path).unwrap().layer_ids.clone();
		for element in ids {
			path.push(element);
			self.render(path, svg);
		}
	}

	/// Wrapper around render, that returns the whole document as a Response.
	pub fn render_root(&mut self) -> String {
		let mut svg = String::new();
		self.render(&mut vec![], &mut svg);
		svg
	}

	fn is_mounted(&self, mount_path: &[LayerId], path: &[LayerId]) -> bool {
		path.starts_with(mount_path) && self.work_mounted
	}

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function respects mounted folders and will thus not contain the layers already
	/// present in the document if a temporary folder is mounted on top.
	pub fn folder(&self, mut path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = self.root.as_folder()?;
		if self.is_mounted(self.work_mount_path.as_slice(), path) {
			path = &path[self.work_mount_path.len()..];
			root = self.work.as_folder()?;
		}
		for id in path {
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function respects mounted folders and will thus not contain the layers already
	/// present in the document if a temporary folder is mounted on top.
	/// If you manually edit the folder you have to set the cache_dirty flag yourself.
	pub fn folder_mut(&mut self, mut path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
		let mut root = if self.is_mounted(self.work_mount_path.as_slice(), path) {
			path = &path[self.work_mount_path.len()..];
			self.work.as_folder_mut()?
		} else {
			self.root.as_folder_mut()?
		};
		for id in path {
			root = root.folder_mut(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function does **not** respect mounted folders and will always return the current
	/// state of the document, disregarding any temporary modifications.
	pub fn document_folder(&self, path: &[LayerId]) -> Result<&Layer, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.as_folder()?.layer(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function does **not** respect mounted folders and will always return the current
	/// state of the document, disregarding any temporary modifications.
	/// If you manually edit the folder you have to set the cache_dirty flag yourself.
	pub fn document_folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.as_folder_mut()?.layer_mut(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	/// Returns a reference to the layer struct at the specified `path`.
	pub fn layer(&self, path: &[LayerId]) -> Result<&Layer, DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder(path)?.layer(id).ok_or(DocumentError::LayerNotFound)
	}

	/// Returns a mutable reference to the layer struct at the specified `path`.
	/// If you manually edit the layer you have to set the cache_dirty flag yourself.
	pub fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or(DocumentError::LayerNotFound)
	}

	/// Replaces the layer at the specified `path` with `layer`.
	pub fn set_layer(&mut self, path: &[LayerId], layer: Layer) -> Result<(), DocumentError> {
		let mut folder = self.root.as_folder_mut()?;
		if let Ok((path, id)) = split_path(path) {
			self.layer_mut(path)?.cache_dirty = true;
			folder = self.folder_mut(path)?;
			if let Some(folder_layer) = folder.layer_mut(id) {
				*folder_layer = layer;
				return Ok(());
			}
		}
		folder.add_layer(layer, -1).ok_or(DocumentError::IndexOutOfBounds)?;
		Ok(())
	}

	/// Adds a new layer to the folder specified by `path`.
	/// Passing a negative `insert_index` indexes relative to the end.
	/// -1 is equivalent to adding the layer to the top.
	pub fn add_layer(&mut self, path: &[LayerId], layer: Layer, insert_index: isize) -> Result<LayerId, DocumentError> {
		let _ = self.layer_mut(path).map(|x| x.cache_dirty = true);
		let folder = self.folder_mut(path)?;
		folder.add_layer(layer, insert_index).ok_or(DocumentError::IndexOutOfBounds)
	}

	/// Deletes the layer specified by `path`.
	pub fn delete(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let (path, id) = split_path(path)?;
		let _ = self.layer_mut(path).map(|x| x.cache_dirty = true);
		self.document_folder_mut(path)?.as_folder_mut()?.remove_layer(id)?;
		Ok(())
	}

	/// Mutate the document by applying the `operation` to it. If the operation necessitates a
	/// reaction from the frontend, responses may be returned.
	pub fn handle_operation(&mut self, operation: Operation) -> Result<Option<Vec<DocumentResponse>>, DocumentError> {
		let responses = match &operation {
			Operation::AddEllipse { path, insert_index, cols, style } => {
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Ellipse(layers::Ellipse::new()), *cols, *style), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::AddRect { path, insert_index, cols, style } => {
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Rect(Rect::new()), *cols, *style), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::AddLine { path, insert_index, cols, style } => {
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Line(Line::new()), *cols, *style), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::AddPen {
				path,
				insert_index,
				points,
				cols,
				style,
			} => {
				let points: Vec<glam::DVec2> = points.iter().map(|&it| it.into()).collect();
				let polyline = PolyLine::new(points);
				self.add_layer(&path, Layer::new(LayerDataTypes::PolyLine(polyline), *cols, *style), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::AddShape {
				path,
				insert_index,
				cols,
				equal_sides,
				sides,
				style,
			} => {
				let s = Shape::new(*equal_sides, *sides);
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Shape(s), *cols, *style), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::DeleteLayer { path } => {
				self.delete(&path)?;

				let (path, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: path.to_vec() }])
			}
			Operation::DuplicateLayer { path } => {
				let layer = self.layer(&path)?.clone();
				let (folder_path, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				let folder = self.folder_mut(folder_path)?;
				folder.add_layer(layer, -1).ok_or(DocumentError::IndexOutOfBounds)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: folder_path.to_vec() }])
			}
			Operation::AddFolder { path } => {
				self.set_layer(&path, Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default()))?;

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: path.clone() }])
			}
			Operation::MountWorkingFolder { path } => {
				self.work_mount_path = path.clone();
				self.work_operations.clear();
				self.work = Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default());
				self.work_mounted = true;
				None
			}
			&Operation::TransformDocument { transform} => {
				let transform = self.root.transform * DAffine2::from_cols_array(&transform);
				self.root.transform = transform;
				self.root.cache_dirty = true;
				self.work.cache_dirty = true;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::DiscardWorkingFolder => {
				self.work_operations.clear();
				self.work_mount_path = vec![];
				self.work = Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default());
				self.work_mounted = false;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::ClearWorkingFolder => {
				self.work_operations.clear();
				self.work = Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default());
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::CommitTransaction => {
				let mut ops = Vec::new();
				let mut path: Vec<LayerId> = vec![];
				std::mem::swap(&mut path, &mut self.work_mount_path);
				std::mem::swap(&mut ops, &mut self.work_operations);
				self.work_mounted = false;
				self.work_mount_path = vec![];
				self.work = Layer::new(LayerDataTypes::Folder(Folder::default()), [1., 0., 0., 1., 0., 0.], PathStyle::default());
				let mut responses = vec![];
				for operation in ops.into_iter() {
					if let Some(mut op_responses) = self.handle_operation(operation)? {
						responses.append(&mut op_responses);
					}
				}
				responses.extend(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path }]);

				Some(responses)
			}
			Operation::ToggleVisibility { path } => {
				let _ = self.layer_mut(&path).map(|layer| {
					layer.visible = !layer.visible;
					layer.cache_dirty = true;
				});
				let path = path.as_slice()[..path.len() - 1].to_vec();
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path }])
			}
		};
		if !matches!(
			operation,
			Operation::CommitTransaction | Operation::MountWorkingFolder { .. } | Operation::DiscardWorkingFolder | Operation::ClearWorkingFolder
		) {
			self.work_operations.push(operation);
		}
		Ok(responses)
	}
}
