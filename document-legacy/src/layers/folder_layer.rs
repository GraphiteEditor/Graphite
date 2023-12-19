use super::layer_info::{LayerData, LegacyLayer, LegacyLayerType};
use super::style::RenderData;
use crate::intersection::Quad;
use crate::{DocumentError, LayerId};

use graphene_core::uuid::generate_uuid;

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are stored.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct FolderLegacyLayer {
	/// The ID that will be assigned to the next layer that is added to the folder
	next_assignment_id: LayerId,
	/// The IDs of the [Layer]s contained within the Folder
	pub layer_ids: Vec<LayerId>,
	/// The [Layer]s contained in the folder
	pub layers: Vec<LegacyLayer>,
}

impl LayerData for FolderLegacyLayer {
	fn render(&mut self, _cache_inner_svg: &mut String, _cache_defs_svg: &mut String, _transforms: &mut Vec<glam::DAffine2>, _render_data: &RenderData) -> bool {
		// Support for rendering folders has been removed as part of the deprecation of legacy layers, as the Layers panel does not support folder thumbnails.

		false
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData) {
		for (layer, layer_id) in self.layers().iter().zip(&self.layer_ids) {
			path.push(*layer_id);
			layer.intersects_quad(quad, path, intersections, render_data);
			path.pop();
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.layers
			.iter()
			.filter_map(|layer| layer.data.bounding_box(transform * layer.transform, render_data))
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
	}
}

impl FolderLegacyLayer {
	/// When a insertion ID is provided, try to insert the layer with the given ID.
	/// If that ID is already used, return `None`.
	/// When no insertion ID is provided, search for the next free ID and insert it with that.
	/// Negative values for `insert_index` represent distance from the end
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::folder_layer::FolderLegacyLayer;
	/// # use graphite_document_legacy::layers::style::PathStyle;
	/// # use graphite_document_legacy::layers::layer_info::LegacyLayerType;
	/// let mut folder = FolderLegacyLayer::default();
	///
	/// // Create two layers to be added to the folder
	/// let mut shape_layer = ShapeLegacyLayer::rectangle(PathStyle::default());
	/// let mut folder_layer = FolderLegacyLayer::default();
	///
	/// folder.add_layer(shape_layer.into(), None, -1);
	/// folder.add_layer(folder_layer.into(), Some(123), 0);
	/// ```
	pub fn add_layer(&mut self, layer: LegacyLayer, id: Option<LayerId>, insert_index: isize) -> Option<LayerId> {
		let mut insert_index = insert_index as i128;

		// Bounds check for the insert index
		if insert_index < 0 {
			insert_index = self.layers.len() as i128 + insert_index + 1;
		}
		if insert_index > self.layers.len() as i128 || insert_index < 0 {
			return None;
		}

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
	}

	/// Remove a layer with a given ID from the folder.
	/// This operation will fail if `id` is not present in the folder.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::folder_layer::FolderLegacyLayer;
	/// let mut folder = FolderLegacyLayer::default();
	///
	/// // Try to remove a layer that does not exist
	/// assert!(folder.remove_layer(123).is_err());
	///
	/// // Add another folder to the folder
	/// folder.add_layer(FolderLegacyLayer::default().into(), Some(123), -1);
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
	pub fn layers(&self) -> &[LegacyLayer] {
		self.layers.as_slice()
	}

	/// Get mutable references to all the [Layer]s in the folder.
	pub fn layers_mut(&mut self) -> &mut [LegacyLayer] {
		self.layers.as_mut_slice()
	}

	pub fn layer(&self, id: LayerId) -> Option<&LegacyLayer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&self.layers[pos])
	}

	pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut LegacyLayer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&mut self.layers[pos])
	}

	pub fn generate_new_folder_ids(&mut self) {
		self.next_assignment_id = generate_uuid();
	}

	/// Returns `true` if the folder contains a layer with the given [LayerId].
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::folder_layer::FolderLegacyLayer;
	/// let mut folder = FolderLegacyLayer::default();
	///
	/// // Search for an id that does not exist
	/// assert!(!folder.folder_contains(123));
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(FolderLegacyLayer::default().into(), Some(123), -1);
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
	/// # use graphite_document_legacy::layers::folder_layer::FolderLegacyLayer;
	/// let mut folder = FolderLegacyLayer::default();
	///
	/// // Search for an id that does not exist
	/// assert!(folder.position_of_layer(123).is_err());
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(FolderLegacyLayer::default().into(), Some(123), -1);
	/// folder.add_layer(FolderLegacyLayer::default().into(), Some(42), -1);
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
	/// # use graphite_document_legacy::layers::folder_layer::FolderLegacyLayer;
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::style::PathStyle;
	/// let mut folder = FolderLegacyLayer::default();
	///
	/// // Search for an id that does not exist
	/// assert!(folder.folder(132).is_none());
	///
	/// // add a folder and search for it
	/// folder.add_layer(FolderLegacyLayer::default().into(), Some(123), -1);
	/// assert!(folder.folder(123).is_some());
	///
	/// // add a non-folder layer and search for it
	/// folder.add_layer(ShapeLegacyLayer::rectangle(PathStyle::default()).into(), Some(42), -1);
	/// assert!(folder.folder(42).is_none());
	/// ```
	pub fn folder(&self, id: LayerId) -> Option<&FolderLegacyLayer> {
		match self.layer(id) {
			Some(LegacyLayer {
				data: LegacyLayerType::Folder(folder),
				..
			}) => Some(folder),
			_ => None,
		}
	}
}
