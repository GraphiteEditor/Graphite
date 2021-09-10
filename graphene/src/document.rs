use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

use crate::{
	layers::{self, Folder, Layer, LayerData, LayerDataType, Shape, Text},
	DocumentError, DocumentResponse, LayerId, Operation, Quad,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Document {
	pub root: Layer,
	#[serde(skip)]
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
	pub fn with_content(serialized_content: &str) -> Result<Self, DocumentError> {
		serde_json::from_str(serialized_content).map_err(|e| DocumentError::InvalidFile(e.to_string()))
	}

	/// Wrapper around render, that returns the whole document as a Response.
	pub fn render_root(&mut self) -> String {
		self.root.render(&mut vec![]);
		self.root.cache.clone()
	}

	pub fn hash(&self) -> u64 {
		self.hasher.finish()
	}

	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
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
	fn folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
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
	fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		if path.is_empty() {
			return Ok(&mut self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or(DocumentError::LayerNotFound)
	}

	pub fn deepest_common_folder<'a>(&self, layers: impl Iterator<Item = &'a [LayerId]>) -> Result<&'a [LayerId], DocumentError> {
		let common_prefix_of_path = self.common_prefix(layers);

		Ok(match self.layer(common_prefix_of_path)?.data {
			LayerDataType::Folder(_) => common_prefix_of_path,
			LayerDataType::Shape(_) => &common_prefix_of_path[..common_prefix_of_path.len() - 1],
			LayerDataType::Text(_) => &common_prefix_of_path[..common_prefix_of_path.len() - 1],
		})
	}

	pub fn common_prefix<'a>(&self, layers: impl Iterator<Item = &'a [LayerId]>) -> &'a [LayerId] {
		layers
			.reduce(|a, b| {
				let number_of_uncommon_ids_in_a = (0..a.len()).position(|i| b.starts_with(&a[..a.len() - i])).unwrap_or_default();
				&a[..(a.len() - number_of_uncommon_ids_in_a)]
			})
			.unwrap_or_default()
	}

	fn serialize_structure(folder: &Folder, structure: &mut Vec<u64>, data: &mut Vec<LayerId>) {
		let mut space = 0;
		for (id, layer) in folder.layer_ids.iter().zip(folder.layers()) {
			data.push(*id);
			match layer.data {
				LayerDataType::Shape(_) => space += 1,
				LayerDataType::Folder(ref folder) => {
					structure.push(space);
					Document::serialize_structure(folder, structure, data);
				}
				LayerDataType::Text(_) => space += 1,
			}
		}
		structure.push(space | 1 << 63);
	}

	/// Serializes the layer structure into a compressed 1d structure
	/// 4,2,1,-2-0,10,12,13,14,15 <- input data
	/// l = 4 = structure.len() <- length of the structure section
	/// structure = 2,1,-2,-0   <- structure section
	/// data = 10,12,13,14,15   <- data section
	///
	/// the numbers in the structure block encode the indentation,
	/// 2 mean read two element from the data section, then place a [
	/// -x means read x elements from the date section an then insert a ]
	///
	/// 2     V 1  V -2  A -0 A
	/// 10,12,  13, 14,15
	/// 10,12,[ 13,[14,15]    ]
	///
	/// resulting layer panel:
	/// 10
	/// 12
	/// [12,13]
	/// [12,13,14]
	/// [12,13,15]
	pub fn serialize_root(&self) -> Vec<LayerId> {
		let (mut structure, mut data) = (vec![0], Vec::new());
		Document::serialize_structure(self.root.as_folder().unwrap(), &mut structure, &mut data);
		structure[0] = structure.len() as u64 - 1;
		structure.extend(data);
		structure
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

	pub fn generate_transform_relative_to_viewport(&self, from: &[LayerId]) -> Result<DAffine2, DocumentError> {
		self.generate_transform_across_scope(from, None)
	}

	pub fn apply_transform_relative_to_viewport(&mut self, layer: &[LayerId], transform: DAffine2) -> Result<(), DocumentError> {
		self.transform_relative_to_scope(layer, None, transform)
	}

	pub fn set_transform_relative_to_viewport(&mut self, layer: &[LayerId], transform: DAffine2) -> Result<(), DocumentError> {
		self.set_transform_relative_to_scope(layer, None, transform)
	}

	fn remove_overlays(&mut self, path: &mut Vec<LayerId>) {
		if self.layer(path).unwrap().overlay {
			self.delete(path).unwrap()
		}
		let ids = self.folder(path).map(|folder| folder.layer_ids.clone()).unwrap_or_default();
		for id in ids {
			path.push(id);
			self.remove_overlays(path);
			path.pop();
		}
	}

	pub fn clone_without_overlays(&self) -> Self {
		let mut document = self.clone();
		document.remove_overlays(&mut vec![]);
		document
	}

	/// Mutate the document by applying the `operation` to it. If the operation necessitates a
	/// reaction from the frontend, responses may be returned.
	pub fn handle_operation(&mut self, operation: &Operation) -> Result<Option<Vec<DocumentResponse>>, DocumentError> {
		operation.pseudo_hash().hash(&mut self.hasher);
		use DocumentResponse::*;

		let responses = match &operation {
			Operation::AddText { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Text(Text::from_string("hello".to_string(), *style)), *transform);

				self.set_layer(path, layer, *insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(path)].concat())
			}
			Operation::AddEllipse { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(Shape::ellipse(*style)), *transform);

				self.set_layer(path, layer, *insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(path)].concat())
			}
			Operation::AddOverlayEllipse { path, transform, style } => {
				let mut ellipse = Shape::ellipse(*style);
				ellipse.render_index = -1;

				let mut layer = Layer::new(LayerDataType::Shape(ellipse), *transform);
				layer.overlay = true;

				self.set_layer(path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }]].concat())
			}
			Operation::AddRect { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(Shape::rectangle(*style)), *transform);

				self.set_layer(path, layer, *insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(path)].concat())
			}
			Operation::AddOverlayRect { path, transform, style } => {
				let mut rect = Shape::rectangle(*style);
				rect.render_index = -1;

				let mut layer = Layer::new(LayerDataType::Shape(rect), *transform);
				layer.overlay = true;

				self.set_layer(path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }]].concat())
			}
			Operation::AddLine { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(Shape::line(*style)), *transform);

				self.set_layer(path, layer, *insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(path)].concat())
			}
			Operation::AddOverlayLine { path, transform, style } => {
				let mut line = Shape::line(*style);
				line.render_index = -1;

				let mut layer = Layer::new(LayerDataType::Shape(line), *transform);
				layer.overlay = true;

				self.set_layer(path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }]].concat())
			}
			Operation::AddNgon {
				path,
				insert_index,
				transform,
				style,
				sides,
			} => {
				let layer = Layer::new(LayerDataType::Shape(Shape::ngon(*sides, *style)), *transform);

				self.set_layer(path, layer, *insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(path)].concat())
			}
			Operation::AddOverlayShape { path, style, bez_path } => {
				let mut shape = Shape::from_bez_path(bez_path.clone(), *style, false);
				shape.render_index = -1;

				let mut layer = Layer::new(LayerDataType::Shape(shape), DAffine2::IDENTITY.to_cols_array());
				layer.overlay = true;

				self.set_layer(path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }]].concat())
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
				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(path)].concat())
			}
			Operation::DeleteLayer { path } => {
				self.delete(path)?;

				let (folder, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				let mut responses = vec![DocumentChanged, DeletedLayer { path: path.clone() }, FolderChanged { path: folder.to_vec() }];
				responses.extend(update_thumbnails_upstream(folder));
				Some(responses)
			}
			Operation::PasteLayer { path, layer, insert_index } => {
				let folder = self.folder_mut(path)?;
				let id = folder.add_layer(layer.clone(), None, *insert_index).ok_or(DocumentError::IndexOutOfBounds)?;
				let full_path = [path.clone(), vec![id]].concat();
				self.mark_as_dirty(&full_path)?;

				let mut responses = vec![DocumentChanged, CreatedLayer { path: full_path }, FolderChanged { path: path.clone() }];
				responses.extend(update_thumbnails_upstream(path));
				Some(responses)
			}
			Operation::DuplicateLayer { path } => {
				let layer = self.layer(path)?.clone();
				let (folder_path, _) = split_path(path.as_slice()).unwrap_or_else(|_| (&[], 0));
				let folder = self.folder_mut(folder_path)?;
				folder.add_layer(layer, None, -1).ok_or(DocumentError::IndexOutOfBounds)?;
				self.mark_as_dirty(&path[..path.len() - 1])?;
				Some(vec![DocumentChanged, FolderChanged { path: folder_path.to_vec() }])
			}
			Operation::RenameLayer { path, name } => {
				self.layer_mut(path)?.name = Some(name.clone());
				Some(vec![LayerChanged { path: path.clone() }])
			}
			Operation::CreateFolder { path } => {
				self.set_layer(path, Layer::new(LayerDataType::Folder(Folder::default()), DAffine2::IDENTITY.to_cols_array()), -1)?;
				self.mark_as_dirty(path)?;

				Some(vec![DocumentChanged, CreatedLayer { path: path.clone() }])
			}
			Operation::TransformLayer { path, transform } => {
				let layer = self.layer_mut(path).unwrap();
				let transform = DAffine2::from_cols_array(transform) * layer.transform;
				layer.transform = transform;
				self.mark_as_dirty(path)?;
				Some(vec![DocumentChanged])
			}
			Operation::TransformLayerInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				self.apply_transform_relative_to_viewport(path, transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerTransformInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				self.set_transform_relative_to_viewport(path, transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetShapePathInViewport { path, bez_path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				self.set_transform_relative_to_viewport(path, transform)?;
				self.mark_as_dirty(path)?;

				match &mut self.layer_mut(path)?.data {
					LayerDataType::Shape(shape) => {
						shape.path = bez_path.clone();
					}
					LayerDataType::Folder(_) => (),
					LayerDataType::Text(text) => todo!(),
				}
				Some(vec![DocumentChanged, LayerChanged { path: path.clone() }])
			}
			Operation::TransformLayerInScope { path, transform, scope } => {
				let transform = DAffine2::from_cols_array(transform);
				let scope = DAffine2::from_cols_array(scope);
				self.transform_relative_to_scope(path, Some(scope), transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerTransformInScope { path, transform, scope } => {
				let transform = DAffine2::from_cols_array(transform);
				let scope = DAffine2::from_cols_array(scope);
				self.set_transform_relative_to_scope(path, Some(scope), transform)?;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerTransform { path, transform } => {
				let transform = DAffine2::from_cols_array(transform);
				let layer = self.layer_mut(path)?;
				layer.transform = transform;
				self.mark_as_dirty(path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::ToggleLayerVisibility { path } => {
				self.mark_as_dirty(path)?;
				let layer = self.layer_mut(path)?;
				layer.visible = !layer.visible;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerVisibility { path, visible } => {
				self.mark_as_dirty(path)?;
				let layer = self.layer_mut(path)?;
				layer.visible = *visible;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerBlendMode { path, blend_mode } => {
				self.mark_as_dirty(path)?;
				self.layer_mut(path)?.blend_mode = *blend_mode;

				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerOpacity { path, opacity } => {
				self.mark_as_dirty(path)?;
				self.layer_mut(path)?.opacity = *opacity;

				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
			}
			Operation::SetLayerStyle { path, style } => {
				let layer = self.layer_mut(path)?;
				match &mut layer.data {
					LayerDataType::Shape(s) => s.style = *style,
					_ => return Err(DocumentError::NotAShape),
				}
				self.mark_as_dirty(path)?;
				Some(vec![DocumentChanged, LayerChanged { path: path.clone() }])
			}
			Operation::SetLayerFill { path, color } => {
				let layer = self.layer_mut(path)?;
				match &mut layer.data {
					LayerDataType::Shape(s) => s.style.set_fill(layers::style::Fill::new(*color)),
					_ => return Err(DocumentError::NotAShape),
				}
				self.mark_as_dirty(path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(path)].concat())
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
		responses.push(DocumentResponse::LayerChanged { path: path[0..(length - i)].to_vec() });
	}
	responses
}
