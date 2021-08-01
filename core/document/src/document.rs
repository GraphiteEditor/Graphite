use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
};

use glam::{DAffine2, DVec2};

use crate::{
	layers::{self, Folder, Layer, LayerData, LayerDataType, Shape},
	DocumentError, DocumentResponse, LayerId, Operation,
};

#[derive(Debug, Clone)]
pub struct Document {
	pub root: Layer,
	pub hasher: DefaultHasher,
}

impl Default for Document {
	fn default() -> Self {
		Self {
			root: Layer::new(LayerDataType::Folder(Folder::default()), DAffine2::IDENTITY.to_cols_array()),
			hasher: DefaultHasher::new(),
		}
	}
}

fn split_path(path: &[LayerId]) -> Result<(&[LayerId], LayerId), DocumentError> {
	let (id, path) = path.split_last().ok_or(DocumentError::InvalidPath)?;
	Ok((path, *id))
}

impl Document {
	/// Wrapper around render, that returns the whole document as a Response.
	pub fn render_root(&mut self) -> String {
		// TODO: remove
		self.mark_as_dirty(&[]);
		self.root.render(&mut vec![]);
		self.root.cache.clone()
	}

	pub fn hash(&self) -> u64 {
		self.hasher.finish()
	}

	/// Checks whether each layer under `path` intersects with the provided `quad` and adds all intersection layers as paths to `intersections`.
	pub fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		self.folder(path).unwrap().intersects_quad(quad, path, intersections);
	}

	/// Checks whether each layer under the root path intersects with the provided `quad` and returns the paths to all intersecting layers.
	pub fn intersects_quad_root(&self, quad: [DVec2; 4]) -> Vec<Vec<LayerId>> {
		let mut intersections = Vec::new();
		self.intersects_quad(quad, &mut vec![], &mut intersections);
		intersections
	}

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function does **not** respect mounted folders and will always return the current
	/// state of the document, disregarding any temporary modifications.
	pub fn folder(&self, path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.as_folder()?.layer(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		root.as_folder()
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// This function does **not** respect mounted folders and will always return the current
	/// state of the document, disregarding any temporary modifications.
	/// If you manually edit the folder you have to set the cache_dirty flag yourself.
	pub fn folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.as_folder_mut()?.layer_mut(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		root.as_folder_mut()
	}

	/// Returns a reference to the layer or folder at the path.
	pub fn layer(&self, path: &[LayerId]) -> Result<&Layer, DocumentError> {
		if path.is_empty() {
			return Ok(&self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder(path)?.layer(id).ok_or(DocumentError::LayerNotFound)
	}

	/// Returns a mutable reference to the layer or folder at the path.
	pub fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		if path.is_empty() {
			return Ok(&mut self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or(DocumentError::LayerNotFound)
	}

	/// Given a path to a layer, returns a vector of the indices in the layer tree
	/// These indices can be used to order a list of layers
	pub fn indices_for_path(&self, path: &[LayerId]) -> Result<Vec<usize>, DocumentError> {
		let mut root = self.root.as_folder()?;
		let mut indices = vec![];
		let (path, layer_id) = split_path(path)?;

		for id in path {
			let pos = root.layer_ids.iter().position(|x| *x == *id).ok_or(DocumentError::LayerNotFound)?;
			indices.push(pos);
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}

		indices.push(root.layer_ids.iter().position(|x| *x == layer_id).ok_or(DocumentError::LayerNotFound)?);

		Ok(indices)
	}

	/// Replaces the layer at the specified `path` with `layer`.
	pub fn set_layer(&mut self, path: &[LayerId], layer: Layer, insert_index: isize) -> Result<(), DocumentError> {
		let mut folder = self.root.as_folder_mut()?;
		let mut layer_id = None;
		if let Ok((path, id)) = split_path(path) {
			layer_id = Some(id);
			self.mark_as_dirty(path)?;
			folder = self.folder_mut(path)?;
			if let Some(folder_layer) = folder.layer_mut(id) {
				*folder_layer = layer;
				return Ok(());
			}
		}
		folder.add_layer(layer, layer_id, insert_index).ok_or(DocumentError::IndexOutOfBounds)?;
		Ok(())
	}

	/// Adds a new layer to the folder specified by `path`.
	/// Passing a negative `insert_index` indexes relative to the end.
	/// -1 is equivalent to adding the layer to the top.
	pub fn add_layer(&mut self, path: &[LayerId], mut layer: Layer, insert_index: isize) -> Result<LayerId, DocumentError> {
		layer.render(&mut self.transforms(path)?);
		let folder = self.folder_mut(path)?;
		folder.add_layer(layer, None, insert_index).ok_or(DocumentError::IndexOutOfBounds)
	}

	/// Deletes the layer specified by `path`.
	pub fn delete(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let (path, id) = split_path(path)?;
		self.mark_as_dirty(path)?;
		self.folder_mut(path)?.remove_layer(id)
	}

	pub fn layer_axis_aligned_bounding_box(&self, path: &[LayerId]) -> Result<Option<[DVec2; 2]>, DocumentError> {
		// TODO: Replace with functions of the transform api
		if path.is_empty() {
			// Special case for root. Root's local is the documents global, so we avoid transforming its transform by itself.
			self.layer_local_bounding_box(path)
		} else {
			let layer = self.layer(path)?;
			Ok(layer.data.bounding_box(self.root.transform * layer.transform))
		}
	}

	pub fn layer_local_bounding_box(&self, path: &[LayerId]) -> Result<Option<[DVec2; 2]>, DocumentError> {
		// TODO: Replace with functions of the transform api
		let layer = self.layer(path)?;
		Ok(layer.data.bounding_box(layer.transform))
	}
	pub fn mark_upstream_as_dirty(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let mut root = &mut self.root;
		root.cache_dirty = true;
		for id in path {
			root = root.as_folder_mut()?.layer_mut(*id).ok_or(DocumentError::LayerNotFound)?;
			root.cache_dirty = true;
		}
		Ok(())
	}

	pub fn mark_downstream_as_dirty(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let mut layer = self.layer_mut(path)?;
		layer.cache_dirty = true;
		let mut path = path.to_vec();
		let len = path.len();
		path.push(0);
		if let Some(ids) = layer.as_folder().ok().map(|f| f.layer_ids.clone()) {
			for id in ids {
				path[len] = id;
				self.mark_downstream_as_dirty(&path)?
			}
		}
		Ok(())
	}

	pub fn mark_as_dirty(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		self.mark_downstream_as_dirty(path)?;
		self.mark_upstream_as_dirty(path)?;
		Ok(())
	}

	pub fn transforms(&self, path: &[LayerId]) -> Result<Vec<DAffine2>, DocumentError> {
		let mut root = &self.root;
		let mut transforms = vec![self.root.transform];
		for id in path {
			if let Ok(folder) = root.as_folder() {
				root = folder.layer(*id).ok_or(DocumentError::LayerNotFound)?;
			}
			transforms.push(root.transform);
		}
		Ok(transforms)
	}

	pub fn multiply_transoforms(&self, path: &[LayerId]) -> Result<DAffine2, DocumentError> {
		let mut root = &self.root;
		let mut trans = self.root.transform;
		for id in path {
			if let Ok(folder) = root.as_folder() {
				root = folder.layer(*id).ok_or(DocumentError::LayerNotFound)?;
			}
			trans = trans * root.transform;
		}
		Ok(trans)
	}

	pub fn generate_transform(&self, from: &[LayerId], to: Option<&[LayerId]>) -> Result<DAffine2, DocumentError> {
		let from_rev = self.multiply_transoforms(from)?.inverse();
		Ok(match to {
			None => from_rev,
			Some(path) => self.multiply_transoforms(path)? * from_rev,
		})
	}

	pub fn transform_in_scope(&mut self, layer: &[LayerId], scope: Option<&[LayerId]>, transform: DAffine2) -> Result<(), DocumentError> {
		let to = self.generate_transform(layer, scope)?;
		let trans = transform * to;
		self.layer_mut(layer)?.transform = to.inverse() * trans;
		Ok(())
	}

	pub fn transform_in_viewport(&mut self, layer: &[LayerId], transform: DAffine2) -> Result<(), DocumentError> {
		self.transform_in_scope(layer, None, transform)
	}

	pub fn transform_layer(&self, path: &[LayerId], to: Option<&[LayerId]>) -> Result<Layer, DocumentError> {
		let transform = self.generate_transform(path, to)?;
		let layer = self.layer(path).unwrap();
		Ok(Layer {
			visible: layer.visible,
			name: layer.name.clone(),
			data: layer.data.clone(),
			transform,
			thumbnail_cache: String::with_capacity(layer.thumbnail_cache.capacity()),
			cache: String::with_capacity(layer.cache.capacity()),
			cache_dirty: true,
			blend_mode: layers::BlendMode::Normal,
			opacity: layer.opacity,
		})
	}

	/// Mutate the document by applying the `operation` to it. If the operation necessitates a
	/// reaction from the frontend, responses may be returned.
	pub fn handle_operation(&mut self, operation: &Operation) -> Result<Option<Vec<DocumentResponse>>, DocumentError> {
		operation.hash(&mut self.hasher);

		let responses = match &operation {
			Operation::AddEllipse { path, insert_index, transform, style } => {
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::ellipse(*style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
			}
			Operation::AddRect { path, insert_index, transform, style } => {
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::rectangle(*style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
			}
			Operation::AddLine { path, insert_index, transform, style } => {
				let id = self.add_layer(path, Layer::new(LayerDataType::Shape(Shape::line(*style)), *transform), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path }])
			}
			Operation::AddPen {
				path,
				insert_index,
				points,
				transform,
				style,
			} => {
				let points: Vec<glam::DVec2> = points.iter().map(|&it| it.into()).collect();
				let id = self.add_layer(path, Layer::new(LayerDataType::Shape(Shape::poly_line(points, *style)), *transform), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path }])
			}
			Operation::AddShape {
				path,
				insert_index,
				transform,
				sides,
				style,
			} => {
				let id = self.add_layer(path, Layer::new(LayerDataType::Shape(Shape::shape(*sides, *style)), *transform), *insert_index)?;
				let path = [path.clone(), vec![id]].concat();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path }])
			}
			Operation::DeleteLayer { path } => {
				self.delete(path)?;

				let (folder, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				Some(vec![
					DocumentResponse::DocumentChanged,
					DocumentResponse::DeletedLayer { path: path.clone() },
					DocumentResponse::FolderChanged { path: folder.to_vec() },
				])
			}
			Operation::PasteLayer { path, layer, insert_index } => {
				let folder = self.folder_mut(path)?;
				//FIXME: This clone of layer should be avoided somehow
				let id = folder.add_layer(layer.clone(), None, *insert_index).ok_or(DocumentError::IndexOutOfBounds)?;
				let full_path = [path.clone(), vec![id]].concat();

				Some(vec![
					DocumentResponse::DocumentChanged,
					DocumentResponse::CreatedLayer { path: full_path },
					DocumentResponse::FolderChanged { path: path.clone() },
				])
			}
			Operation::DuplicateLayer { path } => {
				let layer = self.layer(path)?.clone();
				let (folder_path, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				let folder = self.folder_mut(folder_path)?;
				folder.add_layer(layer, None, -1).ok_or(DocumentError::IndexOutOfBounds)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: folder_path.to_vec() }])
			}
			Operation::RenameLayer { path, name } => {
				self.layer_mut(path)?.name = Some(name.clone());
				Some(vec![DocumentResponse::LayerChanged { path: path.clone() }])
			}
			Operation::AddFolder { path } => {
				self.set_layer(path, Layer::new(LayerDataType::Folder(Folder::default()), DAffine2::IDENTITY.to_cols_array()), -1)?;

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: path.clone() }])
			}
			Operation::TransformLayer { path, transform } => {
				let layer = self.layer_mut(path).unwrap();
				let transform = DAffine2::from_cols_array(transform) * layer.transform;
				layer.transform = transform;
				self.mark_as_dirty(path)?;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::SetLayerTransform { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				let layer = self.layer_mut(path)?;
				layer.transform = transform;
				self.mark_as_dirty(path)?;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::ToggleVisibility { path } => {
				self.mark_as_dirty(path)?;
				if let Ok(layer) = self.layer_mut(path) {
					layer.visible = !layer.visible;
				}
				let path = path.as_slice()[..path.len() - 1].to_vec();
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path }])
			}
			Operation::SetLayerBlendMode { path, blend_mode } => {
				self.mark_as_dirty(path)?;
				self.layer_mut(path)?.blend_mode = *blend_mode;

				let path = path.as_slice()[..path.len() - 1].to_vec();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path }])
			}
			Operation::SetLayerOpacity { path, opacity } => {
				self.mark_as_dirty(path)?;
				self.layer_mut(path)?.opacity = *opacity;

				let path = path.as_slice()[..path.len() - 1].to_vec();

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path }])
			}
			Operation::FillLayer { path, color } => {
				let layer = self.layer_mut(path)?;
				match &mut layer.data {
					LayerDataType::Shape(s) => s.style.set_fill(layers::style::Fill::new(*color)),
					_ => return Err(DocumentError::NotAShape),
				}
				self.mark_as_dirty(path)?;
				Some(vec![DocumentResponse::DocumentChanged])
			}
		};
		Ok(responses)
	}
}
