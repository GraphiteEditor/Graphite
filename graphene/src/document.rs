use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
};

use glam::{DAffine2, DVec2};

use crate::{
	layers::{self, Folder, Layer, LayerData, LayerDataType, Shape},
	DocumentError, DocumentResponse, LayerId, Operation, Quad,
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

impl Document {
	/// Wrapper around render, that returns the whole document as a Response.
	pub fn render_root(&mut self) -> String {
		self.root.render(&mut vec![]);
		self.root.cache.clone()
	}

	pub fn hash(&self) -> u64 {
		self.hasher.finish()
	}

	/// Checks whether each layer under `path` intersects with the provided `quad` and adds all intersection layers as paths to `intersections`.
	pub fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		self.layer(path).unwrap().intersects_quad(quad, path, intersections);
	}

	/// Checks whether each layer under the root path intersects with the provided `quad` and returns the paths to all intersecting layers.
	pub fn intersects_quad_root(&self, quad: Quad) -> Vec<Vec<LayerId>> {
		let mut intersections = Vec::new();
		self.intersects_quad(quad, &mut vec![], &mut intersections);
		intersections
	}

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	pub fn folder(&self, path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.as_folder()?.layer(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		root.as_folder()
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
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
	pub fn add_layer(&mut self, path: &[LayerId], layer: Layer, insert_index: isize) -> Result<LayerId, DocumentError> {
		let folder = self.folder_mut(path)?;
		folder.add_layer(layer, None, insert_index).ok_or(DocumentError::IndexOutOfBounds)
	}

	/// Deletes the layer specified by `path`.
	pub fn delete(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let (path, id) = split_path(path)?;
		self.mark_as_dirty(path)?;
		self.folder_mut(path)?.remove_layer(id)
	}

	pub fn visible_layers(&self, path: &mut Vec<LayerId>, paths: &mut Vec<Vec<LayerId>>) -> Result<(), DocumentError> {
		if !self.layer(path)?.visible {
			return Ok(());
		}
		if let Ok(folder) = self.folder(path) {
			for layer in folder.layer_ids.iter() {
				path.push(*layer);
				self.visible_layers(path, paths)?;
				path.pop();
			}
		} else {
			paths.push(path.clone());
		}
		Ok(())
	}

	pub fn viewport_bounding_box(&self, path: &[LayerId]) -> Result<Option<[DVec2; 2]>, DocumentError> {
		let layer = self.layer(path)?;
		let transform = self.multiply_transforms(path)?;
		Ok(layer.data.bounding_box(transform))
	}

	pub fn visible_layers_bounding_box(&self) -> Option<[DVec2; 2]> {
		let mut paths = vec![];
		self.visible_layers(&mut vec![], &mut paths).ok()?;
		self.combined_viewport_bounding_box(paths.iter().map(|x| x.as_slice()))
	}

	pub fn combined_viewport_bounding_box<'a>(&self, paths: impl Iterator<Item = &'a [LayerId]>) -> Option<[DVec2; 2]> {
		let boxes = paths.filter_map(|path| self.viewport_bounding_box(path).ok()?);
		boxes.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
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

	pub fn multiply_transforms(&self, path: &[LayerId]) -> Result<DAffine2, DocumentError> {
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

	pub fn generate_transform_across_scope(&self, from: &[LayerId], to: Option<DAffine2>) -> Result<DAffine2, DocumentError> {
		let from_rev = self.multiply_transforms(from)?;
		let scope = to.unwrap_or(DAffine2::IDENTITY);
		Ok(scope * from_rev)
	}

	pub fn transform_relative_to_scope(&mut self, layer: &[LayerId], scope: Option<DAffine2>, transform: DAffine2) -> Result<(), DocumentError> {
		let to = self.generate_transform_across_scope(&layer[..layer.len() - 1], scope)?;
		let layer = self.layer_mut(layer)?;
		layer.transform = to.inverse() * transform * to * layer.transform;
		Ok(())
	}

	pub fn set_transform_relative_to_scope(&mut self, layer: &[LayerId], scope: Option<DAffine2>, transform: DAffine2) -> Result<(), DocumentError> {
		let to = self.generate_transform_across_scope(&layer[..layer.len() - 1], scope)?;
		let layer = self.layer_mut(layer)?;
		layer.transform = to.inverse() * transform;
		Ok(())
	}

	pub fn apply_transform_relative_to_viewport(&mut self, layer: &[LayerId], transform: DAffine2) -> Result<(), DocumentError> {
		self.transform_relative_to_scope(layer, None, transform)
	}

	pub fn set_transform_relative_to_viewport(&mut self, layer: &[LayerId], transform: DAffine2) -> Result<(), DocumentError> {
		self.set_transform_relative_to_scope(layer, None, transform)
	}

	/// Mutate the document by applying the `operation` to it. If the operation necessitates a
	/// reaction from the frontend, responses may be returned.
	pub fn handle_operation(&mut self, operation: &Operation) -> Result<Option<Vec<DocumentResponse>>, DocumentError> {
		operation.pseudo_hash().hash(&mut self.hasher);

		let responses = match &operation {
			Operation::AddEllipse { path, insert_index, transform, style } => {
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::ellipse(*style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
			}
			Operation::AddRect { path, insert_index, transform, style } => {
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::rectangle(*style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
			}
			Operation::AddBoundingBox { path, transform, style } => {
				let mut rect = Shape::rectangle(*style);
				rect.render_index = -1;
				self.set_layer(path, Layer::new(LayerDataType::Shape(rect), *transform), -1)?;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::AddShape {
				path,
				insert_index,
				transform,
				style,
				sides,
			} => {
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::shape(*sides, *style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
			}
			Operation::AddLine { path, insert_index, transform, style } => {
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::line(*style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
			}
			Operation::AddPen {
				path,
				insert_index,
				points,
				transform,
				style,
			} => {
				let points: Vec<glam::DVec2> = points.iter().map(|&it| it.into()).collect();
				self.set_layer(path, Layer::new(LayerDataType::Shape(Shape::poly_line(points, *style)), *transform), *insert_index)?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::CreatedLayer { path: path.clone() }])
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
				let id = folder.add_layer(layer.clone(), None, *insert_index).ok_or(DocumentError::IndexOutOfBounds)?;
				let full_path = [path.clone(), vec![id]].concat();
				self.mark_as_dirty(&full_path)?;

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
				self.mark_as_dirty(&path[..path.len() - 1])?;
				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: folder_path.to_vec() }])
			}
			Operation::RenameLayer { path, name } => {
				self.layer_mut(path)?.name = Some(name.clone());
				Some(vec![DocumentResponse::LayerChanged { path: path.clone() }])
			}
			Operation::CreateFolder { path } => {
				self.set_layer(path, Layer::new(LayerDataType::Folder(Folder::default()), DAffine2::IDENTITY.to_cols_array()), -1)?;
				self.mark_as_dirty(path)?;

				Some(vec![DocumentResponse::DocumentChanged, DocumentResponse::FolderChanged { path: path.clone() }])
			}
			Operation::TransformLayer { path, transform } => {
				let layer = self.layer_mut(path).unwrap();
				let transform = DAffine2::from_cols_array(transform) * layer.transform;
				layer.transform = transform;
				self.mark_as_dirty(path)?;
				Some(vec![DocumentResponse::DocumentChanged])
			}
			Operation::TransformLayerInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				self.apply_transform_relative_to_viewport(path, transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerTransformInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				self.set_transform_relative_to_viewport(path, transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::TransformLayerInScope { path, transform, scope } => {
				let transform = DAffine2::from_cols_array(transform);
				let scope = DAffine2::from_cols_array(scope);
				self.transform_relative_to_scope(path, Some(scope), transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerTransformInScope { path, transform, scope } => {
				let transform = DAffine2::from_cols_array(transform);
				let scope = DAffine2::from_cols_array(scope);
				self.set_transform_relative_to_scope(path, Some(scope), transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerTransform { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				let layer = self.layer_mut(path)?;
				layer.transform = transform;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::ToggleVisibility { path } => {
				self.mark_as_dirty(path)?;
				if let Ok(layer) = self.layer_mut(path) {
					layer.visible = !layer.visible;
				}
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerBlendMode { path, blend_mode } => {
				self.mark_as_dirty(path)?;
				self.layer_mut(path)?.blend_mode = *blend_mode;

				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerOpacity { path, opacity } => {
				self.mark_as_dirty(path)?;
				self.layer_mut(path)?.opacity = *opacity;

				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::FillLayer { path, color } => {
				let layer = self.layer_mut(path)?;
				match &mut layer.data {
					LayerDataType::Shape(s) => s.style.set_fill(layers::style::Fill::new(*color)),
					_ => return Err(DocumentError::NotAShape),
				}
				self.mark_as_dirty(path)?;
				Some([vec![DocumentResponse::DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
		};
		Ok(responses)
	}
}

fn split_path(path: &[LayerId]) -> Result<(&[LayerId], LayerId), DocumentError> {
	let (id, path) = path.split_last().ok_or(DocumentError::InvalidPath)?;
	Ok((path, *id))
}

fn update_thumbnails_upstream(path: &[LayerId]) -> Vec<DocumentResponse> {
	let length = path.len();
	let mut responses = Vec::with_capacity(length);
	for i in 0..length {
		responses.push(DocumentResponse::LayerChanged {
			path: path[(length - i)..length].to_vec(),
		});
	}
	responses
}
