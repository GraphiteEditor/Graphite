use super::layer_info::{Layer, LayerData, LayerDataType};
use super::style::RenderData;
use crate::intersection::Quad;
use crate::layers::text_layer::FontCache;
use crate::{DocumentError, LayerId};

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are
/// stored in the [layers](FolderLayer::layers) field.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct FolderLayer {
	/// The ID that will be assigned to the next layer that is added to the folder
	next_assignment_id: LayerId,
	/// The IDs of the [Layer]s contained within the Folder
	pub layer_ids: Vec<LayerId>,
	/// The [Layer]s contained in the folder
	pub layers: Vec<Layer>,
}

impl LayerData for FolderLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: RenderData) -> bool {
		let mut any_child_requires_redraw = false;
		for layer in &mut self.layers {
			let (svg_value, requires_redraw) = layer.render(transforms, svg_defs, render_data);
			*svg += svg_value;
			any_child_requires_redraw = any_child_requires_redraw || requires_redraw;
		}
		any_child_requires_redraw
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, font_cache: &FontCache) {
		for (layer, layer_id) in self.layers().iter().zip(&self.layer_ids) {
			path.push(*layer_id);
			layer.intersects_quad(quad, path, intersections, font_cache);
			path.pop();
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2, font_cache: &FontCache) -> Option<[DVec2; 2]> {
		self.layers
			.iter()
			.filter_map(|layer| layer.data.bounding_box(transform * layer.transform, font_cache))
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
	}
}

impl FolderLayer {
	/// When a insertion ID is provided, try to insert the layer with the given ID.
	/// If that ID is already used, return `None`.
	/// When no insertion ID is provided, search for the next free ID and insert it with that.
	/// Negative values for `insert_index` represent distance from the end
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLayer;
	/// # use graphite_document_legacy::layers::folder_layer::FolderLayer;
	/// # use graphite_document_legacy::layers::style::PathStyle;
	/// # use graphite_document_legacy::layers::layer_info::LayerDataType;
	/// let mut folder = FolderLayer::default();
	///
	/// // Create two layers to be added to the folder
	/// let mut shape_layer = ShapeLayer::rectangle(PathStyle::default());
	/// let mut folder_layer = FolderLayer::default();
	///
	/// folder.add_layer(shape_layer.into(), None, -1);
	/// folder.add_layer(folder_layer.into(), Some(123), 0);
	/// ```
	pub fn add_layer(&mut self, layer: Layer, id: Option<LayerId>, insert_index: isize) -> Option<LayerId> {
		let mut insert_index = insert_index as i128;

		if insert_index < 0 {
			insert_index = self.layers.len() as i128 + insert_index as i128 + 1;
		}

		if insert_index <= self.layers.len() as i128 && insert_index >= 0 {
			if let Some(id) = id {
				self.next_assignment_id = id;
			}
			if self.layer_ids.contains(&self.next_assignment_id) {
				return None;
			}

			let id = self.next_assignment_id;
			self.layers.insert(insert_index as usize, layer);
			self.layer_ids.insert(insert_index as usize, id);

			// Linear probing for collision avoidance
			while self.layer_ids.contains(&self.next_assignment_id) {
				self.next_assignment_id += 1;
			}

			Some(id)
		} else {
			None
		}
	}

	/// Remove a layer with a given ID from the folder.
	/// This operation will fail if `id` is not present in the folder.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::folder_layer::FolderLayer;
	/// let mut folder = FolderLayer::default();
	///
	/// // Try to remove a layer that does not exist
	/// assert!(folder.remove_layer(123).is_err());
	///
	/// // Add another folder to the folder
	/// folder.add_layer(FolderLayer::default().into(), Some(123), -1);
	///
	/// // Try to remove that folder again
	/// assert!(folder.remove_layer(123).is_ok());
	/// assert_eq!(folder.layers().len(), 0)
	/// ```
	pub fn remove_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
		let pos = self.position_of_layer(id)?;
		self.layers.remove(pos);
		self.layer_ids.remove(pos);
		Ok(())
	}

	/// Returns a list of [LayerId]s in the folder.
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	/// Get references to all the [Layer]s in the folder.
	pub fn layers(&self) -> &[Layer] {
		self.layers.as_slice()
	}

	/// Get mutable references to all the [Layer]s in the folder.
	pub fn layers_mut(&mut self) -> &mut [Layer] {
		self.layers.as_mut_slice()
	}

	pub fn layer(&self, id: LayerId) -> Option<&Layer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&self.layers[pos])
	}

	pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&mut self.layers[pos])
	}

	/// Returns `true` if the folder contains a layer with the given [LayerId].
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::folder_layer::FolderLayer;
	/// let mut folder = FolderLayer::default();
	///
	/// // Search for an id that does not exist
	/// assert!(!folder.folder_contains(123));
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(FolderLayer::default().into(), Some(123), -1);
	///
	/// // Search for the id "123"
	/// assert!(folder.folder_contains(123));
	/// ```
	pub fn folder_contains(&self, id: LayerId) -> bool {
		self.layer_ids.contains(&id)
	}

	/// Tries to find the index of a layer with the given [LayerId] within the folder.
	/// This operation will fail if no layer with a matching ID is present in the folder.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::folder_layer::FolderLayer;
	/// let mut folder = FolderLayer::default();
	///
	/// // Search for an id that does not exist
	/// assert!(folder.position_of_layer(123).is_err());
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(FolderLayer::default().into(), Some(123), -1);
	/// folder.add_layer(FolderLayer::default().into(), Some(42), -1);
	///
	/// assert_eq!(folder.position_of_layer(123), Ok(0));
	/// assert_eq!(folder.position_of_layer(42), Ok(1));
	/// ```
	pub fn position_of_layer(&self, layer_id: LayerId) -> Result<usize, DocumentError> {
		self.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound([layer_id].into()))
	}

	/// Tries to get a reference to a folder with the given [LayerId].
	/// This operation will return `None` if either no layer with `id` exists
	/// in the folder, or the layer with matching ID is not a folder.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::folder_layer::FolderLayer;
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLayer;
	/// # use graphite_document_legacy::layers::style::PathStyle;
	/// let mut folder = FolderLayer::default();
	///
	/// // Search for an id that does not exist
	/// assert!(folder.folder(132).is_none());
	///
	/// // add a folder and search for it
	/// folder.add_layer(FolderLayer::default().into(), Some(123), -1);
	/// assert!(folder.folder(123).is_some());
	///
	/// // add a non-folder layer and search for it
	/// folder.add_layer(ShapeLayer::rectangle(PathStyle::default()).into(), Some(42), -1);
	/// assert!(folder.folder(42).is_none());
	/// ```
	pub fn folder(&self, id: LayerId) -> Option<&FolderLayer> {
		match self.layer(id) {
			Some(Layer {
				data: LayerDataType::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}

	/// Tries to get a mutable reference to folder with the given `id`.
	/// This operation will return `None` if either no layer with `id` exists
	/// in the folder or the layer with matching ID is not a folder.
	/// See the [FolderLayer::folder] method for a usage example.
	pub fn folder_mut(&mut self, id: LayerId) -> Option<&mut FolderLayer> {
		match self.layer_mut(id) {
			Some(Layer {
				data: LayerDataType::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}
}
