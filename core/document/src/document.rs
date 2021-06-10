use crate::{
	layers::{self, Folder, Layer, LayerData, LayerDataTypes, Line, PolyLine, Rect, Shape},
	DocumentError, DocumentResponse, LayerId, Operation,
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
			self.document_folder_mut(path).unwrap().render(svg);
			self.work.render(svg);
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

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function respects mounted folders and will thus not contain the layers already
	/// present in the document if a temporary folder is mounted on top.
	/// If you manually edit the folder you have to set the cache_dirty flag yourself.
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

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function does **not** respect mounted folders and will always return the current
	/// state of the document, disregarding any temporary modifications.
	pub fn document_folder(&self, path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function does **not** respect mounted folders and will always return the current
	/// state of the document, disregarding any temporary modifications.
	/// If you manually edit the folder you have to set the cache_dirty flag yourself.
	pub fn document_folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.folder_mut(*id).ok_or(DocumentError::LayerNotFound)?;
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
		let mut folder = &mut self.root;
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
		self.document_folder_mut(path)?.remove_layer(id)?;
		Ok(())
	}

	/// Mutate the document by applying the `operation` to it. If the operation necessitates a
	/// reaction from the frontend, responses may be returned.
	pub fn handle_operation(&mut self, operation: Operation) -> Result<Option<Vec<DocumentResponse>>, DocumentError> {
		let responses = match &operation {
			Operation::AddCircle { path, insert_index, cx, cy, r, style } => {
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Circle(layers::Circle::new((*cx, *cy), *r, *style))), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::AddEllipse {
				path,
				insert_index,
				cx,
				cy,
				rx,
				ry,
				rot,
				style,
			} => {
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Ellipse(layers::Ellipse::new((*cx, *cy), (*rx, *ry), *rot, *style))), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
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
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Rect(Rect::new((*x0, *y0), (*x1, *y1), *style))), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
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
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Line(Line::new((*x0, *y0), (*x1, *y1), *style))), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::AddPen { path, insert_index, points, style } => {
				let points: Vec<kurbo::Point> = points.iter().map(|&it| it.into()).collect();
				let polyline = PolyLine::new(points, *style);
				self.add_layer(&path, Layer::new(LayerDataTypes::PolyLine(polyline)), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged])
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
				let s = Shape::new((*x0, *y0), (*x1, *y1), *sides, *style);
				let id = self.add_layer(&path, Layer::new(LayerDataTypes::Shape(s)), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::SelectLayer { path }])
			}
			Operation::DeleteLayer { path } => {
				self.delete(&path)?;

				let (path, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: path.to_vec() }])
			}
			Operation::AddFolder { path } => {
				self.set_layer(&path, Layer::new(LayerDataTypes::Folder(Folder::default())))?;

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: path.clone() }])
			}
			Operation::MountWorkingFolder { path } => {
				self.work_mount_path = path.clone();
				self.work_operations.clear();
				self.work = Folder::default();
				self.work_mounted = true;
				None
			}
			Operation::DiscardWorkingFolder => {
				self.work_operations.clear();
				self.work_mount_path = vec![];
				self.work = Folder::default();
				self.work_mounted = false;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::ClearWorkingFolder => {
				self.work_operations.clear();
				self.work = Folder::default();
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::CommitTransaction => {
				let mut ops = Vec::new();
				let mut path: Vec<LayerId> = vec![];
				std::mem::swap(&mut path, &mut self.work_mount_path);
				std::mem::swap(&mut ops, &mut self.work_operations);
				self.work_mounted = false;
				self.work_mount_path = vec![];
				self.work = Folder::default();
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
