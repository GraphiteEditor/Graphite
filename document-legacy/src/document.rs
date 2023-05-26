use crate::intersection::Quad;
use crate::layers::folder_layer::FolderLayer;
use crate::layers::layer_info::{Layer, LayerData, LayerDataType, LayerDataTypeDiscriminant};
use crate::layers::layer_layer::{CachedOutputData, LayerLayer};
use crate::layers::shape_layer::ShapeLayer;
use crate::layers::style::RenderData;
use crate::{DocumentError, DocumentResponse, Operation};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::vec;

/// A number that identifies a layer.
/// This does not technically need to be unique globally, only within a folder.
pub type LayerId = u64;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Document {
	/// The root layer, usually a [FolderLayer](layers::folder_layer::FolderLayer) that contains all other [Layers](layers::layer_info::Layer).
	pub root: Layer,
	/// The state_identifier serves to provide a way to uniquely identify a particular state that the document is in.
	/// This identifier is not a hash and is not guaranteed to be equal for equivalent documents.
	#[serde(skip)]
	pub state_identifier: DefaultHasher,
	#[serde(default)]
	pub document_network: graph_craft::document::NodeNetwork,
}

impl PartialEq for Document {
	fn eq(&self, other: &Self) -> bool {
		self.state_identifier.finish() == other.state_identifier.finish()
	}
}

impl Default for Document {
	fn default() -> Self {
		Self {
			root: Layer::new(LayerDataType::Folder(FolderLayer::default()), DAffine2::IDENTITY.to_cols_array()),
			state_identifier: DefaultHasher::new(),
			document_network: {
				use graph_craft::document::{value::TaggedValue, NodeInput, NodeNetwork};
				let mut network = NodeNetwork::default();
				let node = graph_craft::document::DocumentNode {
					name: "Output".into(),
					inputs: vec![NodeInput::value(TaggedValue::GraphicGroup(Default::default()), true)],
					implementation: graph_craft::document::DocumentNodeImplementation::Unresolved("graphene_core::ops::IdNode".into()),
					metadata: graph_craft::document::DocumentNodeMetadata::position((8, 4)),
					..Default::default()
				};
				network.push_node(node, false);
				network
			},
		}
	}
}

impl Document {
	/// Wrapper around render, that returns the whole document as a Response.
	pub fn render_root(&mut self, render_data: &RenderData) -> String {
		// Render and append to the defs section
		let mut svg_defs = String::from("<defs>");
		self.root.render(&mut vec![], &mut svg_defs, render_data);
		svg_defs.push_str("</defs>");

		// Append the cached rendered SVG
		svg_defs.push_str(&self.root.cache);

		svg_defs
	}

	/// Renders everything below the given layer contained within its parent folder.
	pub fn render_layers_below(&mut self, below_layer_path: &[LayerId], render_data: &RenderData) -> Option<String> {
		// Split the path into the layer ID and its parent folder
		let (layer_id_to_render_below, parent_folder_path) = below_layer_path.split_last()?;

		// Note: it is bad practice to directly clone and modify the document structure, this is a temporary hack until this whole system is replaced by the node graph
		let mut temp_subset_folder = self.layer_mut(parent_folder_path).ok()?.clone();
		if let LayerDataType::Folder(ref mut folder) = temp_subset_folder.data {
			// Remove the upper layers to leave behind the lower subset for rendering
			let count_of_layers_below = folder.layer_ids.iter().position(|id| id == layer_id_to_render_below).unwrap();
			folder.layer_ids.truncate(count_of_layers_below);
			folder.layers.truncate(count_of_layers_below);

			// Render and append to the defs section
			let mut svg_defs = String::from("<defs>");
			temp_subset_folder.render(&mut vec![], &mut svg_defs, render_data);
			svg_defs.push_str("</defs>");

			// Append the cached rendered SVG
			svg_defs.push_str(&temp_subset_folder.cache);

			Some(svg_defs)
		} else {
			None
		}
	}

	/// Renders a layer and its children
	pub fn render_layer(&mut self, layer_path: &[LayerId], render_data: &RenderData) -> Option<String> {
		// Note: it is bad practice to directly clone and modify the document structure, this is a temporary hack until this whole system is replaced by the node graph
		let mut temp_clone = self.layer_mut(layer_path).ok()?.clone();

		// Render and append to the defs section
		let mut svg_defs = String::from("<defs>");
		temp_clone.render(&mut vec![], &mut svg_defs, render_data);
		svg_defs.push_str("</defs>");

		// Append the cached rendered SVG
		svg_defs.push_str(&temp_clone.cache);

		Some(svg_defs)
	}

	pub fn current_state_identifier(&self) -> u64 {
		self.state_identifier.finish()
	}

	/// Checks whether each layer under `path` intersects with the provided `quad` and adds all intersection layers as paths to `intersections`.
	pub fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData) {
		self.layer(path).unwrap().intersects_quad(quad, path, intersections, render_data);
	}

	/// Checks whether each layer under the root path intersects with the provided `quad` and returns the paths to all intersecting layers.
	pub fn intersects_quad_root(&self, quad: Quad, render_data: &RenderData) -> Vec<Vec<LayerId>> {
		let mut intersections = Vec::new();
		self.intersects_quad(quad, &mut vec![], &mut intersections, render_data);
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
		self.folder(path)?.layer(id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))
	}

	/// Returns a mutable reference to the layer or folder at the path.
	pub fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		if path.is_empty() {
			return Ok(&mut self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))
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
		if let Ok(folder) = self.folder(path) {
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

	pub fn viewport_bounding_box(&self, path: &[LayerId], render_data: &RenderData) -> Result<Option<[DVec2; 2]>, DocumentError> {
		let layer = self.layer(path)?;
		let transform = self.multiply_transforms(path)?;
		Ok(layer.data.bounding_box(transform, render_data))
	}

	pub fn bounding_box_and_transform(&self, path: &[LayerId], render_data: &RenderData) -> Result<Option<([DVec2; 2], DAffine2)>, DocumentError> {
		let layer = self.layer(path)?;
		let transform = self.multiply_transforms(&path[..path.len() - 1])?;
		Ok(layer.data.bounding_box(layer.transform, render_data).map(|bounds| (bounds, transform)))
	}

	/// Compute the center of transformation multiplied with `Document::multiply_transforms`.
	pub fn pivot(&self, path: &[LayerId], render_data: &RenderData) -> Option<DVec2> {
		let layer = self.layer(path).ok()?;
		Some(self.multiply_transforms(path).unwrap_or_default().transform_point2(layer.layerspace_pivot(render_data)))
	}

	pub fn visible_layers_bounding_box(&self, render_data: &RenderData) -> Option<[DVec2; 2]> {
		let mut paths = vec![];
		self.visible_layers(&mut vec![], &mut paths).ok()?;
		self.combined_viewport_bounding_box(paths.iter().map(|x| x.as_slice()), render_data)
	}

	pub fn combined_viewport_bounding_box<'a>(&self, paths: impl Iterator<Item = &'a [LayerId]>, render_data: &RenderData) -> Option<[DVec2; 2]> {
		let boxes = paths.filter_map(|path| self.viewport_bounding_box(path, render_data).ok()?);
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

	/// For the purposes of rendering, this invalidates the render cache for the layer so it must be re-rendered next time.
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

				let mut responses = vec![
					DocumentChanged,
					CreatedLayer {
						path: path.clone(),
						is_selected: true,
					},
				];
				responses.extend(update_thumbnails_upstream(&path));

				Some(responses)
			}
			Operation::AddRect { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(ShapeLayer::rectangle(style)), transform);

				self.set_layer(&path, layer, insert_index)?;

				let mut responses = vec![
					DocumentChanged,
					CreatedLayer {
						path: path.clone(),
						is_selected: true,
					},
				];
				responses.extend(update_thumbnails_upstream(&path));

				Some(responses)
			}
			Operation::AddLine { path, insert_index, transform, style } => {
				let layer = Layer::new(LayerDataType::Shape(ShapeLayer::line(style)), transform);

				self.set_layer(&path, layer, insert_index)?;

				let mut responses = vec![
					DocumentChanged,
					CreatedLayer {
						path: path.clone(),
						is_selected: true,
					},
				];
				responses.extend(update_thumbnails_upstream(&path));

				Some(responses)
			}
			Operation::AddFrame {
				path,
				insert_index,
				transform,
				network,
			} => {
				let layer = Layer::new(LayerDataType::Layer(LayerLayer { network, ..Default::default() }), transform);

				self.set_layer(&path, layer, insert_index)?;

				let mut responses = vec![
					DocumentChanged,
					CreatedLayer {
						path: path.clone(),
						is_selected: true,
					},
				];
				responses.extend(update_thumbnails_upstream(&path));

				Some(responses)
			}
			Operation::SetLayerPreserveAspect { layer_path, preserve_aspect } => {
				if let Ok(layer) = self.layer_mut(&layer_path) {
					layer.preserve_aspect = preserve_aspect;
				}
				Some(vec![LayerChanged { path: layer_path.clone() }])
			}
			Operation::AddShape {
				path,
				transform,
				insert_index,
				style,
				subpath,
			} => {
				let shape = ShapeLayer::new(subpath, style);
				self.set_layer(&path, Layer::new(LayerDataType::Shape(shape), transform), insert_index)?;
				Some(vec![DocumentChanged, CreatedLayer { path, is_selected: true }])
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

				let mut responses = vec![
					DocumentChanged,
					CreatedLayer {
						path: path.clone(),
						is_selected: true,
					},
				];
				responses.extend(update_thumbnails_upstream(&path));

				Some(responses)
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
				duplicating,
			} => {
				let (folder_path, layer_id) = split_path(&destination_path)?;
				let mut responses = vec![DocumentChanged];

				// If we are duplicating, use the parent layer path as the folder we insert to
				let (created_layer_path, folder_changed_path) = if duplicating {
					let folder = self.folder_mut(&destination_path)?;
					let new_layer_id = folder.add_layer(*layer, None, insert_index).ok_or(DocumentError::IndexOutOfBounds)?;

					([destination_path.as_slice(), &[new_layer_id]].concat(), destination_path.clone())
				} else {
					let folder = self.folder_mut(folder_path)?;
					folder.add_layer(*layer, Some(layer_id), insert_index).ok_or(DocumentError::IndexOutOfBounds)?;

					(destination_path.clone(), folder_path.to_vec())
				};

				responses.push(CreatedLayer {
					path: created_layer_path,
					is_selected: !duplicating,
				});
				responses.push(FolderChanged { path: folder_changed_path.to_vec() });
				responses.extend(update_thumbnails_upstream(&destination_path));

				self.mark_as_dirty(&destination_path)?;

				// Recursively iterate through each layer in a folder and add it to the responses vector
				fn aggregate_insertions(folder: &FolderLayer, path: &mut Vec<LayerId>, responses: &mut Vec<DocumentResponse>, duplicating: bool) {
					for (id, layer) in folder.layer_ids.iter().zip(folder.layers()) {
						path.push(*id);

						responses.push(DocumentResponse::CreatedLayer {
							path: path.clone(),
							is_selected: !duplicating,
						});
						if let LayerDataType::Folder(f) = &layer.data {
							aggregate_insertions(f, path, responses, duplicating);
						}

						path.pop();
					}
				}

				if let Ok(folder) = self.folder(&destination_path) {
					aggregate_insertions(folder, &mut destination_path.as_slice().to_vec(), &mut responses, duplicating);
				};

				Some(responses)
			}
			Operation::DuplicateLayer { path } => {
				// Notes for review: I wasn't sure on how to apply unwrap_or() to lines of code that use the function self.layer()

				let layer = self.layer(&path)?.clone();
				let layer_is_folder = layer.as_folder().is_ok();
				let (folder_path, _) = split_path(path.as_slice()).unwrap_or((&[], 0));

				// Recursively collect each of the nested folders and shapes if the layer is a folder
				fn recursive_collect(document: &mut Document, layer_path: &[u64]) -> Vec<Vec<u64>> {
					let mut duplicated_layers_so_far = Vec::new();

					let children = document.folder_children_paths(layer_path);
					for child in children {
						if document.is_folder(&child) {
							duplicated_layers_so_far.push(child.to_vec());
							duplicated_layers_so_far.append(&mut recursive_collect(document, &child));
						} else {
							duplicated_layers_so_far.push(child);
						}
					}

					duplicated_layers_so_far
				}
				let mut duplicated_layers = if layer_is_folder { recursive_collect(self, &path) } else { Vec::new() };

				// Iterate through each layer path and collect the corresponding Layer objects into one vector
				let duplicated_layers_objects = duplicated_layers
					.iter()
					.map(|layer_path| self.layer(layer_path.as_slice()).cloned().ok())
					.collect::<Option<Vec<_>>>()
					.ok_or(DocumentError::InvalidPath)?;

				// Sort both vectors by the layer path depth, from shallowest (fewest) to deepest (most)
				let mut indices: Vec<usize> = (0..duplicated_layers.len()).collect();
				indices.sort_by_key(|&i| duplicated_layers[i].len());
				duplicated_layers.sort_by_key(|a| a.len());
				let duplicate_layer_objects_sorted: Vec<&Layer> = indices.iter().map(|&i| &duplicated_layers_objects[i]).collect();

				let folder = self.folder_mut(folder_path)?;

				let selected_id = path.last().copied().unwrap_or_default();
				let insert_index = folder.layer_ids.iter().position(|&id| id == selected_id).unwrap_or(0) as isize + 1;

				if let Some(new_layer_id) = folder.add_layer(layer, None, insert_index) {
					let new_path = [folder_path, &[new_layer_id]].concat();

					let mut responses = vec![
						DocumentChanged,
						CreatedLayer {
							path: new_path.clone(),
							is_selected: true,
						},
						FolderChanged { path: folder_path.to_vec() },
					];
					responses.extend(update_thumbnails_upstream(path.as_slice()));

					if layer_is_folder {
						let new_folder = self.folder_mut(&new_path)?;
						// Clear the new folders layer_ids/layers because they contain the layer_ids/layers of the layers that were duplicated
						new_folder.layer_ids = vec![];
						new_folder.layers = vec![];
						// Generate a new next assignment ID to avoid collision
						new_folder.generate_new_folder_ids();

						let mut old_to_new_layer_id: HashMap<LayerId, LayerId> = HashMap::new();

						for (i, duplicate_layer) in duplicated_layers.into_iter().enumerate() {
							let Some(old_layer_id) = duplicate_layer.last().cloned() else { continue; };

							// Iterate through each ID of the current duplicate layer
							// If the dictionary contains the ID, we know the duplicate folder has been created already. Use the existing layer ID instead of creating a new one
							let sub_layer = &duplicate_layer[new_path.len()..];
							let new_sub_path: Vec<u64> = sub_layer.iter().filter_map(|id| old_to_new_layer_id.get(id).cloned()).collect();

							// Combine the new path with the IDs of the duplicate layer path to create the path where we insert the duplicate layer
							let mut updated_layer = duplicate_layer_objects_sorted.get(i).unwrap().to_owned().clone();
							let updated_layer_path_parent = [new_path.clone(), new_sub_path].concat();

							// Clear the new folder's layer_ids and layers because they contain the layer_ids/layers of the layer were duplicated
							if self.is_folder(duplicate_layer) {
								let updated_layer_as_folder: &mut FolderLayer = updated_layer.as_folder_mut()?;
								updated_layer_as_folder.layer_ids = vec![];
								updated_layer_as_folder.layers = vec![];
								updated_layer_as_folder.generate_new_folder_ids()
							}

							let result = self
								.handle_operation(Operation::InsertLayer {
									layer: Box::new(updated_layer),
									destination_path: updated_layer_path_parent,
									insert_index: -1,
									duplicating: true,
								})
								.ok()
								.flatten()
								.unwrap_or_default();

							// Collect the new ID of the duplicated layer from the InsertLayer Operation
							// Map the layer ID of the layer we're duplicating to the new ID
							if let DocumentResponse::CreatedLayer { path, .. } = result.get(1).unwrap() {
								if let Some(new_layer_id) = path.last() {
									old_to_new_layer_id.entry(old_layer_id).or_insert(*new_layer_id);
								}
							}

							responses.extend(result);
						}
					}

					self.mark_as_dirty(folder_path)?;

					Some(responses)
				} else {
					return Err(DocumentError::IndexOutOfBounds);
				}
			}
			Operation::RenameLayer { layer_path: path, new_name: name } => {
				self.layer_mut(&path)?.name = Some(name);
				Some(vec![LayerChanged { path }])
			}
			Operation::CreateFolder { path, insert_index } => {
				self.set_layer(&path, Layer::new(LayerDataType::Folder(FolderLayer::default()), DAffine2::IDENTITY.to_cols_array()), insert_index)?;
				self.mark_as_dirty(&path)?;

				let mut responses = vec![
					DocumentChanged,
					CreatedLayer {
						path: path.clone(),
						is_selected: true,
					},
				];
				responses.extend(update_thumbnails_upstream(&path));

				Some(responses)
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
			Operation::SetLayerBlobUrl { layer_path, blob_url, resolution: _ } => {
				let layer = self.layer_mut(&layer_path).unwrap_or_else(|_| panic!("Blob URL for invalid layer with path '{:?}'", layer_path));

				let LayerDataType::Layer(layer) = &mut layer.data else {
					panic!("Incorrectly trying to set the image blob URL for a layer that is not a 'Layer' layer type");
				};

				layer.cached_output_data = CachedOutputData::BlobURL(blob_url);

				self.mark_as_dirty(&layer_path)?;
				Some([vec![DocumentChanged, LayerChanged { path: layer_path.clone() }], update_thumbnails_upstream(&layer_path)].concat())
			}
			Operation::ClearBlobURL { path } => {
				let layer = self.layer_mut(&path).expect("Clearing node graph image for invalid layer");
				match &mut layer.data {
					LayerDataType::Layer(layer) => {
						if matches!(layer.cached_output_data, CachedOutputData::BlobURL(_)) {
							layer.cached_output_data = CachedOutputData::None;
						}
					}
					e => panic!("Incorrectly trying to clear the blob URL for layer of type {}", LayerDataTypeDiscriminant::from(&*e)),
				}
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged, LayerChanged { path: path.clone() }], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetPivot { layer_path, pivot } => {
				let layer = self.layer_mut(&layer_path).expect("Setting pivot for invalid layer");
				layer.pivot = pivot.into();
				self.mark_as_dirty(&layer_path)?;
				Some([vec![DocumentChanged, LayerChanged { path: layer_path.clone() }], update_thumbnails_upstream(&layer_path)].concat())
			}
			Operation::SetLayerTransformInViewport { path, transform } => {
				let transform = DAffine2::from_cols_array(&transform);
				self.set_transform_relative_to_viewport(&path, transform)?;
				self.mark_as_dirty(&path)?;
				Some([vec![DocumentChanged], update_thumbnails_upstream(&path)].concat())
			}
			Operation::SetShapePath { path, subpath } => {
				self.mark_as_dirty(&path)?;

				if let LayerDataType::Shape(shape) = &mut self.layer_mut(&path)?.data {
					shape.shape = subpath;
				}
				Some(vec![DocumentChanged, LayerChanged { path }])
			}
			Operation::SetVectorData { path, vector_data } => {
				if let LayerDataType::Layer(layer) = &mut self.layer_mut(&path)?.data {
					layer.cached_output_data = CachedOutputData::VectorPath(Box::new(vector_data));
				}
				Some(Vec::new())
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
			Operation::SetLayerScaleAroundPivot { path, new_scale } => {
				let layer = self.layer_mut(&path)?;

				let matrix = layer.transform.to_cols_array();
				let old_scale = (matrix[0], matrix[3]);

				let scale_factor = DVec2::from(new_scale) / DVec2::from(old_scale);

				let offset = DAffine2::from_translation(-layer.pivot);
				let scale = DAffine2::from_scale(scale_factor);
				let offset_back = DAffine2::from_translation(layer.pivot);
				layer.transform = layer.transform * offset_back * scale * offset;

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
					_ => return Err(DocumentError::NotShape),
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

pub fn pick_layer_safe_imaginate_resolution(layer: &Layer, render_data: &RenderData) -> (u64, u64) {
	let layer_bounds = layer.bounding_transform(render_data);
	let layer_bounds_size = (layer_bounds.transform_vector2((1., 0.).into()).length(), layer_bounds.transform_vector2((0., 1.).into()).length());

	pick_safe_imaginate_resolution(layer_bounds_size)
}

pub fn pick_safe_imaginate_resolution((width, height): (f64, f64)) -> (u64, u64) {
	const MAX_RESOLUTION: u64 = 1000 * 1000;

	let mut scale_factor = 1.;

	let round_to_increment = |size: f64| (size / 64.).round() as u64 * 64;

	loop {
		let possible_solution = (round_to_increment(width * scale_factor), round_to_increment(height * scale_factor));

		if possible_solution.0 * possible_solution.1 <= MAX_RESOLUTION {
			return possible_solution;
		}

		scale_factor -= 0.1;
	}
}
