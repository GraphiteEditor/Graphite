use crate::boolean_ops::boolean_operation;
use crate::intersection::Quad;
use crate::layers;
use crate::layers::folder_layer::FolderLayer;
use crate::layers::image_layer::ImageLayer;
use crate::layers::layer_info::{Layer, LayerData, LayerDataType};
use crate::layers::shape_layer::ShapeLayer;
use crate::layers::style::ViewMode;
use crate::layers::text_layer::TextLayer;
use crate::{DocumentError, DocumentResponse, Operation};

use glam::{DAffine2, DVec2};
use kurbo::Affine;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A number that identifies a layer.
/// This does not technically need to be unique globally, only within a folder.
pub type LayerId = u64;
pub type FontCache<'a> = &'a HashMap<String, Vec<u8>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Document {
	/// The root layer, usually a [FolderLayer](layers::folder_layer::FolderLayer) that contains all other [Layers](layers::layer_info::Layer).
	pub root: Layer,
	/// The state_identifier serves to provide a way to uniquely identify a particular state that the document is in.
	/// This identifier is not a hash and is not guaranteed to be equal for equivalent documents.
	#[serde(skip)]
	pub state_identifier: DefaultHasher,
	pub font_cache: HashMap<String, Vec<u8>>,
}

impl Default for Document {
	fn default() -> Self {
		Self {
			root: Layer::new(LayerDataType::Folder(FolderLayer::default()), DAffine2::IDENTITY.to_cols_array()),
			state_identifier: DefaultHasher::new(),
			font_cache: HashMap::new(),
		}
	}
}

impl Document {
	/// Wrapper around render, that returns the whole document as a Response.
	pub fn render_root(&mut self, mode: ViewMode) -> String {
		let mut svg_defs = String::from("<defs>");

		self.root.render(&mut vec![], mode, &mut svg_defs, &self.font_cache);

		svg_defs.push_str("</defs>");

		svg_defs.push_str(&self.root.cache);
		svg_defs
	}

	pub fn current_state_identifier(&self) -> u64 {
		self.state_identifier.finish()
	}

	/// Checks whether each layer under `path` intersects with the provided `quad` and adds all intersection layers as paths to `intersections`.
	pub fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		self.layer(path).unwrap().intersects_quad(quad, path, intersections, &self.font_cache);
	}

	/// Checks whether each layer under the root path intersects with the provided `quad` and returns the paths to all intersecting layers.
	pub fn intersects_quad_root(&self, quad: Quad) -> Vec<Vec<LayerId>> {
		let mut intersections = Vec::new();
		self.intersects_quad(quad, &mut vec![], &mut intersections);
		intersections
	}

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	pub fn folder(&self, path: impl AsRef<[LayerId]>) -> Result<&FolderLayer, DocumentError> {
		let mut root = &self.root;
		for id in path.as_ref() {
			root = root.as_folder()?.layer(*id).ok_or_else(|| DocumentError::LayerNotFound(path.as_ref().into()))?;
		}
		root.as_folder()
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	/// If you manually edit the folder you have to set the cache_dirty flag yourself.
	fn folder_mut(&mut self, path: &[LayerId]) -> Result<&mut FolderLayer, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.as_folder_mut()?.layer_mut(*id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
		}
		root.as_folder_mut()
	}

	/// Returns a reference to the layer or folder at the path.
	pub fn layer(&self, path: &[LayerId]) -> Result<&Layer, DocumentError> {
		if path.is_empty() {
			return Ok(&self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder(&path)?.layer(id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))
	}

	/// Returns a mutable reference to the layer or folder at the path.
	fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		if path.is_empty() {
			return Ok(&mut self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))
	}

	/// Returns vector `Shape`s for each specified in `paths`.
	/// If any path is not a shape, or does not exist, `DocumentError::InvalidPath` is returned.
	fn transformed_shapes(&self, paths: &[Vec<LayerId>]) -> Result<Vec<ShapeLayer>, DocumentError> {
		let mut shapes: Vec<ShapeLayer> = Vec::new();
		let undo_viewport = self.root.transform.inverse();
		for path in paths {
			match (self.multiply_transforms(path), &self.layer(path)?.data) {
				(Ok(shape_transform), LayerDataType::Shape(shape)) => {
					let mut new_shape = shape.clone();
					new_shape.path.apply_affine(Affine::new((undo_viewport * shape_transform).to_cols_array()));
					shapes.push(new_shape);
				}
				(Ok(_), _) => return Err(DocumentError::InvalidPath),
				(Err(err), _) => return Err(err),
			}
		}
		Ok(shapes)
	}

	pub fn common_layer_path_prefix<'a>(&self, layers: impl Iterator<Item = &'a [LayerId]>) -> &'a [LayerId] {
		layers.reduce(|a, b| &a[..a.iter().zip(b.iter()).take_while(|&(a, b)| a == b).count()]).unwrap_or_default()
	}

	/// Filters out the non folders from an iterator of paths.
	/// Takes and Iterator over &[LayerId] or &Vec<LayerId>.
	pub fn folders<'a, T>(&'a self, layers: impl Iterator<Item = T> + 'a) -> impl Iterator<Item = T> + 'a
	where
		T: AsRef<[LayerId]> + std::cmp::Ord + 'a,
	{
		layers.filter(|layer| self.is_folder(layer.as_ref()))
	}

	/// Returns the shallowest folder given the selection, even if the selection doesn't contain any folders
	pub fn shallowest_common_folder<'a>(&self, layers: impl Iterator<Item = &'a [LayerId]>) -> Result<&'a [LayerId], DocumentError> {
		let common_prefix_of_path = self.common_layer_path_prefix(layers);

		Ok(match self.layer(common_prefix_of_path)?.data {
			LayerDataType::Folder(_) => common_prefix_of_path,
			_ => &common_prefix_of_path[..common_prefix_of_path.len() - 1],
		})
	}

	/// Returns all folders that are not contained in any other of the given folders
	/// Takes and Iterator over &[LayerId] or &Vec<LayerId>.
	pub fn shallowest_folders<'a, T>(&'a self, layers: impl Iterator<Item = T>) -> Vec<T>
	where
		T: AsRef<[LayerId]> + std::cmp::Ord + 'a,
	{
		Self::shallowest_unique_layers(self.folders(layers))
	}

	/// Returns all layers that are not contained in any other of the given folders
	/// Takes and Iterator over &[LayerId] or &Vec<LayerId>.
	pub fn shallowest_unique_layers<'a, T>(layers: impl Iterator<Item = T>) -> Vec<T>
	where
		T: AsRef<[LayerId]> + std::cmp::Ord + 'a,
	{
		let mut sorted_layers: Vec<_> = layers.collect();
		sorted_layers.sort();
		// Sorting here creates groups of similar UUID paths
		sorted_layers.dedup_by(|a, b| a.as_ref().starts_with(b.as_ref()));
		sorted_layers
	}
	/// Deepest to shallowest (longest to shortest path length)
	/// Takes and Iterator over &[LayerId] or &Vec<LayerId>.
	pub fn sorted_folders_by_depth<'a, T>(&'a self, layers: impl Iterator<Item = T>) -> Vec<T>
	where
		T: AsRef<[LayerId]> + std::cmp::Ord + 'a,
	{
		let mut folders: Vec<_> = self.folders(layers).collect();
		folders.sort_by_key(|a| std::cmp::Reverse(a.as_ref().len()));
		folders
	}

	pub fn folder_children_paths(&self, path: &[LayerId]) -> Vec<Vec<LayerId>> {
		if let Ok(folder) = self.folder(&path) {
			folder.list_layers().iter().map(|f| [path, &[*f]].concat()).collect()
		} else {
			vec![]
		}
	}

	pub fn is_folder(&self, path: impl AsRef<[LayerId]>) -> bool {
		return self.folder(path.as_ref()).is_ok();
	}

	// Determines which layer is closer to the root, if path_a return true, if path_b return false
	// Answers the question: Is A closer to the root than B?
	pub fn layer_closer_to_root(&self, path_a: &[u64], path_b: &[u64]) -> bool {
		// Convert UUIDs to indices
		let indices_for_path_a = self.indices_for_path(path_a).unwrap();
		let indices_for_path_b = self.indices_for_path(path_b).unwrap();

		let longest = max(indices_for_path_a.len(), indices_for_path_b.len());
		for i in 0..longest {
			// usize::MAX becomes negative one here, sneaky. So folders are compared as [X, -1]. This is intentional.
			let index_a = *indices_for_path_a.get(i).unwrap_or(&usize::MAX) as i32;
			let index_b = *indices_for_path_b.get(i).unwrap_or(&usize::MAX) as i32;

			// At the point at which the two paths first differ, compare to see which is closer to the root
			if index_a != index_b {
				// If index_a is smaller, index_a is closer to the root
				return index_a < index_b;
			}
		}

		false
	}

	// Is  the target layer between a <-> b layers, inclusive
	pub fn layer_is_between(&self, target: &[u64], path_a: &[u64], path_b: &[u64]) -> bool {
		// If the target is the root, it isn't between
		if target.is_empty() {
			return false;
		}

		// This function is inclusive, so we consider path_a, path_b to be between themselves
		if target == path_a || target == path_b {
			return true;
		};

		// These can't both be true and be between two values
		let layer_vs_a = self.layer_closer_to_root(target, path_a);
		let layer_vs_b = self.layer_closer_to_root(target, path_b);

		// To be in-between you need to be above A and below B or vice versa
		layer_vs_a != layer_vs_b
	}

	/// Given a path to a layer, returns a vector of the indices in the layer tree
	/// These indices can be used to order a list of layers
	pub fn indices_for_path(&self, path: &[LayerId]) -> Result<Vec<usize>, DocumentError> {
		let mut root = self.root.as_folder()?;
		let mut indices = vec![];
		let (path, layer_id) = split_path(path)?;

		// TODO: appears to be n^2? should we maintain a lookup table?
		for id in path {
			let pos = root.layer_ids.iter().position(|x| *x == *id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
			indices.push(pos);
			root = root.folder(*id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
		}

		indices.push(root.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?);

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

	/// Visit each layer recursively, marks all children as dirty
	pub fn mark_children_as_dirty(layer: &mut Layer) -> bool {
		match layer.data {
			LayerDataType::Folder(ref mut folder) => {
				for sub_layer in folder.layers_mut() {
					if Document::mark_children_as_dirty(sub_layer) {
						layer.cache_dirty = true;
					}
				}
			}
			_ => layer.cache_dirty = true,
		}
		layer.cache_dirty
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
		if let Ok(folder) = self.folder(&path) {
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
		Ok(layer.data.bounding_box(transform, &self.font_cache))
	}

	pub fn bounding_box_and_transform(&self, path: &[LayerId]) -> Result<Option<([DVec2; 2], DAffine2)>, DocumentError> {
		let layer = self.layer(path)?;
		let transform = self.multiply_transforms(&path[..path.len() - 1])?;
		Ok(layer.data.bounding_box(layer.transform, &self.font_cache).map(|bounds| (bounds, transform)))
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

	/// Mark the layer at the provided path, as well as all the folders containing it, as dirty.
	pub fn mark_upstream_as_dirty(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let mut root = &mut self.root;
		root.cache_dirty = true;
		for id in path {
			root = root.as_folder_mut()?.layer_mut(*id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
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
				root = folder.layer(*id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
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
				root = folder.layer(*id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
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

	/// Mutate the document by applying the `operation` to it. If the operation necessitates a
	/// reaction from the frontend, responses may be returned.
	pub fn handle_operation(&mut self, operation: Operation) -> Result<Option<Vec<DocumentResponse>>, DocumentError> {
		use DocumentResponse::*;

		operation.pseudo_hash().hash(&mut self.state_identifier);

		let responses = match operation {
			Operation::AddEllipse { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(ShapeLayer::ellipse(style)), transform);

				self.set_layer(&path, layer, insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::AddOverlayEllipse { path, transform, style } => {
				let mut ellipse = ShapeLayer::ellipse(style);
				ellipse.render_index = -1;

				let layer = Layer::new(LayerDataType::Shape(ellipse), transform);
				self.set_layer(&path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path }]].concat())
			}
			Operation::AddRect { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(ShapeLayer::rectangle(style)), transform);

				self.set_layer(&path, layer, insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::AddOverlayRect { path, transform, style } => {
				let mut rect = ShapeLayer::rectangle(style);
				rect.render_index = -1;

				let layer = Layer::new(LayerDataType::Shape(rect), transform);
				self.set_layer(&path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path }]].concat())
			}
			Operation::AddLine { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(ShapeLayer::line(style)), transform);

				self.set_layer(&path, layer, insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::AddOverlayLine { path, transform, style } => {
				let mut line = ShapeLayer::line(style);
				line.render_index = -1;

				let layer = Layer::new(LayerDataType::Shape(line), transform);
				self.set_layer(&path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path }]].concat())
			}
			Operation::AddText {
				path,
				insert_index,
				transform,
				text,

				style,
				size,
				font_name,
				font_file,
			} => {
				let layer = Layer::new(LayerDataType::Text(TextLayer::new(text, style, size, font_name, font_file, &self.font_cache)), transform);

				self.set_layer(&path, layer, insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::AddImage {
				path,
				transform,
				insert_index,
				image_data,
				mime,
			} => {
				let layer = Layer::new(LayerDataType::Image(ImageLayer::new(mime, image_data)), transform);

				self.set_layer(&path, layer, insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetTextEditability { path, editable } => {
				self.layer_mut(&path)?.as_text_mut()?.editable = editable;
				self.mark_as_dirty(&path)?;
				Some(vec![DocumentChanged])
			}
			Operation::SetTextContent { path, new_text } => {
				// Not using Document::layer_mut is necessary because we alson need to borrow the font cache
				let mut current_folder = &mut self.root;

				let (layer_path, id) = split_path(&path)?;
				for id in layer_path {
					current_folder = current_folder.as_folder_mut()?.layer_mut(*id).ok_or_else(|| DocumentError::LayerNotFound(layer_path.into()))?;
				}
				current_folder
					.as_folder_mut()?
					.layer_mut(id)
					.ok_or_else(|| DocumentError::LayerNotFound(path.clone()))?
					.as_text_mut()?
					.update_text(new_text, &self.font_cache);

				self.mark_as_dirty(&path)?;

				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::AddNgon {
				path,
				insert_index,
				transform,
				style,
				sides,
			} => {
				let layer = Layer::new(LayerDataType::Shape(ShapeLayer::ngon(sides, style)), transform);

				self.set_layer(&path, layer, insert_index)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::AddOverlayShape { path, style, bez_path, closed } => {
				let mut shape = ShapeLayer::from_bez_path(bez_path, style, closed);
				shape.render_index = -1;

				let layer = Layer::new(LayerDataType::Shape(shape), DAffine2::IDENTITY.to_cols_array());
				self.set_layer(&path, layer, -1)?;

				Some([vec![DocumentChanged, CreatedLayer { path }]].concat())
			}
			Operation::AddShape {
				path,
				transform,
				insert_index,
				style,
				bez_path,
				closed,
			} => {
				let shape = ShapeLayer::from_bez_path(bez_path, style, closed);
				self.set_layer(&path, Layer::new(LayerDataType::Shape(shape), transform), insert_index)?;
				Some([vec![DocumentChanged, CreatedLayer { path }]].concat())
			}
			Operation::AddPolyline {
				path,
				insert_index,
				points,
				transform,
				style,
			} => {
				let points: Vec<glam::DVec2> = points.iter().map(|&it| it.into()).collect();
				self.set_layer(&path, Layer::new(LayerDataType::Shape(ShapeLayer::poly_line(points, style)), transform), insert_index)?;
				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::BooleanOperation { operation, selected } => {
				// TODO: proper difference
				// TODO: proper style selection (done?)
				// TODO: should generate symmetrical code
				// TODO: Operations on any number of shapes
				// TODO: boolean ops on any number of shapes
				// TODO: handle overlapping identical curve case
				// TODO: precision reached without intersection bug (maybe caused by separating a closed path, or dragging handles)
				// TODO: click on shape should drag the shape
				// TODO: add ability to undo
				let mut responses = Vec::new();
				if selected.len() > 1 && selected.len() < 3 {
					// ? apparently `selected` should be reversed
					let mut shapes = self.transformed_shapes(&selected)?;
					let mut shape_drain = shapes.drain(..).rev();
					let new_shapes = boolean_operation(operation, shape_drain.next().unwrap(), shape_drain.next().unwrap())?;

					for path in selected {
						self.delete(&path)?;
						responses.push(DocumentResponse::DeletedLayer { path })
					}
					for new_shape in new_shapes {
						let new_id = self.add_layer(&[], Layer::new(LayerDataType::Shape(new_shape), DAffine2::IDENTITY.to_cols_array()), -1)?;
						responses.push(DocumentResponse::CreatedLayer { path: vec![new_id] })
					}
				}
				Some([vec![DocumentChanged, DocumentResponse::FolderChanged { path: vec![] }], responses].concat())
			}
			Operation::AddSpline {
				path,
				insert_index,
				points,
				transform,
				style,
			} => {
				let points: Vec<glam::DVec2> = points.iter().map(|&it| it.into()).collect();
				self.set_layer(&path, Layer::new(LayerDataType::Shape(ShapeLayer::spline(points, style)), transform), insert_index)?;
				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::DeleteLayer { path } => {
				fn aggregate_deletions(folder: &FolderLayer, path: &mut Vec<LayerId>, responses: &mut Vec<DocumentResponse>) {
					for (id, layer) in folder.layer_ids.iter().zip(folder.layers()) {
						path.push(*id);
						responses.push(DocumentResponse::DeletedLayer { path: path.clone() });
						if let LayerDataType::Folder(f) = &layer.data {
							aggregate_deletions(f, path, responses);
						}
						path.pop();
					}
				}
				let mut responses = Vec::new();
				if let Ok(folder) = self.folder(&path) {
					aggregate_deletions(folder, &mut path.clone(), &mut responses)
				};
				self.delete(&path)?;

				let (folder, _) = split_path(path.as_slice()).unwrap_or((&[], 0));
				responses.extend([DocumentChanged, DeletedLayer { path: path.clone() }, FolderChanged { path: folder.to_vec() }]);
				responses.extend(update_thumbnails_upstream(folder));
				Some(responses)
			}
			Operation::InsertLayer {
				destination_path,
				layer,
				insert_index,
			} => {
				let (folder_path, layer_id) = split_path(&destination_path)?;
				let folder = self.folder_mut(folder_path)?;
				folder.add_layer(layer, Some(layer_id), insert_index).ok_or(DocumentError::IndexOutOfBounds)?;
				self.mark_as_dirty(&destination_path)?;

				fn aggregate_insertions(folder: &FolderLayer, path: &mut Vec<LayerId>, responses: &mut Vec<DocumentResponse>) {
					for (id, layer) in folder.layer_ids.iter().zip(folder.layers()) {
						path.push(*id);
						responses.push(DocumentResponse::CreatedLayer { path: path.clone() });
						if let LayerDataType::Folder(f) = &layer.data {
							aggregate_insertions(f, path, responses);
						}
						path.pop();
					}
				}

				let mut responses = Vec::new();
				if let Ok(folder) = self.folder(&destination_path) {
					aggregate_insertions(folder, &mut destination_path.as_slice().to_vec(), &mut responses)
				};

				responses.extend([DocumentChanged, CreatedLayer { path: destination_path.clone() }, FolderChanged { path: folder_path.to_vec() }]);
				responses.extend(update_thumbnails_upstream(&destination_path));
				Some(responses)
			}
			Operation::DuplicateLayer { path } => {
				let layer = self.layer(&path)?.clone();
				let (folder_path, _) = split_path(path.as_slice()).unwrap_or((&[], 0));
				let folder = self.folder_mut(folder_path)?;
				if let Some(new_layer_id) = folder.add_layer(layer, None, -1) {
					let new_path = [folder_path, &[new_layer_id]].concat();
					self.mark_as_dirty(folder_path)?;
					Some(
						[
							vec![DocumentChanged, CreatedLayer { path: new_path }, FolderChanged { path: folder_path.to_vec() }],
							update_thumbnails_upstream(path.as_slice()),
						]
						.concat(),
					)
				} else {
					return Err(DocumentError::IndexOutOfBounds);
				}
			}
			Operation::ModifyFont { path, name, file, size } => {
				// Not using Document::layer_mut is necessary because we alson need to borrow the font cache
				let mut current_folder = &mut self.root;
				let (folder_path, id) = split_path(&path)?;
				for id in folder_path {
					current_folder = current_folder.as_folder_mut()?.layer_mut(*id).ok_or_else(|| DocumentError::LayerNotFound(folder_path.into()))?;
				}
				let layer_mut = current_folder.as_folder_mut()?.layer_mut(id).ok_or_else(|| DocumentError::LayerNotFound(folder_path.into()))?;
				let text = layer_mut.as_text_mut()?;

				text.font = name;
				text.font_file = file;
				text.size = size;
				text.regenerate_path(text.load_face(&self.font_cache));
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged, LayerChanged { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::RenameLayer { layer_path: path, new_name: name } => {
				self.layer_mut(&path)?.name = Some(name);
				Some(vec![LayerChanged { path }])
			}
			Operation::CreateFolder { path } => {
				self.set_layer(&path, Layer::new(LayerDataType::Folder(FolderLayer::default()), DAffine2::IDENTITY.to_cols_array()), -1)?;
				self.mark_as_dirty(&path)?;

				Some([vec![DocumentChanged, CreatedLayer { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::TransformLayer { path, transform } => {
				let layer = self.layer_mut(&path).unwrap();
				let transform = DAffine2::from_cols_array(&transform) * layer.transform;
				layer.transform = transform;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::TransformLayerInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(&transform);
				self.apply_transform_relative_to_viewport(&path, transform)?;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetImageBlobUrl { path, blob_url, dimensions } => {
				let image = self.layer_mut(&path).expect("Blob url for invalid layer").as_image_mut().unwrap();
				image.blob_url = Some(blob_url);
				image.dimensions = dimensions.into();
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged, LayerChanged { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}

			Operation::SetLayerTransformInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(&transform);
				self.set_transform_relative_to_viewport(&path, transform)?;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetShapePath { path, bez_path } => {
				self.mark_as_dirty(&path)?;

				if let LayerDataType::Shape(shape) = &mut self.layer_mut(&path)?.data {
					shape.path = bez_path;
				}
				Some(vec![DocumentChanged, LayerChanged { path }])
			}
			Operation::SetShapePathInViewport { path, bez_path, transform } => {
				let transform = DAffine2::from_cols_array(&transform);
				self.set_transform_relative_to_viewport(&path, transform)?;
				self.mark_as_dirty(&path)?;

				// Not using Document::layer_mut is necessary because we alson need to borrow the font cache
				let mut current_folder = &mut self.root;
				let (folder_path, id) = split_path(&path)?;
				for id in folder_path {
					current_folder = current_folder.as_folder_mut()?.layer_mut(*id).ok_or_else(|| DocumentError::LayerNotFound(folder_path.into()))?;
				}
				let layer_mut = current_folder.as_folder_mut()?.layer_mut(id).ok_or_else(|| DocumentError::LayerNotFound(folder_path.into()))?;

				if let LayerDataType::Text(t) = &mut layer_mut.data {
					let bezpath = t.to_bez_path(t.load_face(&self.font_cache));
					layer_mut.data = layers::layer_info::LayerDataType::Shape(ShapeLayer::from_bez_path(bezpath, t.style.clone(), true));
				}

				if let LayerDataType::Shape(shape) = &mut layer_mut.data {
					shape.path = bez_path;
				}
				Some([vec![DocumentChanged, LayerChanged { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::TransformLayerInScope { path, transform, scope } => {
				let transform = DAffine2::from_cols_array(&transform);
				let scope = DAffine2::from_cols_array(&scope);
				self.transform_relative_to_scope(&path, Some(scope), transform)?;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerTransformInScope { path, transform, scope } => {
				let transform = DAffine2::from_cols_array(&transform);
				let scope = DAffine2::from_cols_array(&scope);
				self.set_transform_relative_to_scope(&path, Some(scope), transform)?;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerTransform { path, transform } => {
				let transform = DAffine2::from_cols_array(&transform);
				let layer = self.layer_mut(&path)?;
				layer.transform = transform;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::ToggleLayerVisibility { path } => {
				self.mark_as_dirty(&path)?;
				let layer = self.layer_mut(&path)?;
				layer.visible = !layer.visible;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerVisibility { path, visible } => {
				self.mark_as_dirty(&path)?;
				let layer = self.layer_mut(&path)?;
				layer.visible = visible;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerName { path, name } => {
				self.mark_as_dirty(&path)?;
				let mut layer = self.layer_mut(&path)?;
				layer.name = if name.as_str() == "" { None } else { Some(name) };

				Some(vec![LayerChanged { path }])
			}
			Operation::SetLayerBlendMode { path, blend_mode } => {
				self.mark_as_dirty(&path)?;
				self.layer_mut(&path)?.blend_mode = blend_mode;

				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerOpacity { path, opacity } => {
				self.mark_as_dirty(&path)?;
				self.layer_mut(&path)?.opacity = opacity;

				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerStyle { path, style } => {
				let layer = self.layer_mut(&path)?;
				match &mut layer.data {
					LayerDataType::Shape(s) => s.style = style,
					LayerDataType::Text(text) => text.style = style,
					_ => return Err(DocumentError::NotAShape),
				}
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged, LayerChanged { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerStroke { path, stroke } => {
				let layer = self.layer_mut(&path)?;
				layer.style_mut()?.set_stroke(stroke);
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetLayerFill { path, fill } => {
				let layer = self.layer_mut(&path)?;
				layer.style_mut()?.set_fill(fill);
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
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
