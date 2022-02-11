use super::layer_info::{Layer, LayerData, LayerDataType};
use super::style::ViewMode;
use crate::intersection::Quad;
use crate::{DocumentError, LayerId};

use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are
/// added to the folder.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct Folder {
	/// The id that will be assigned to the next layer that
	/// is added to the folder
	next_assignment_id: LayerId,
	/// The ID's of the Layers contained within the Folder
	pub layer_ids: Vec<LayerId>,
	/// The layers contained in the folder
	layers: Vec<Layer>,
}

impl LayerData for Folder {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<glam::DAffine2>, view_mode: ViewMode) {
		for layer in &mut self.layers {
			let _ = writeln!(svg, "{}", layer.render(transforms, view_mode));
		}
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		for (layer, layer_id) in self.layers().iter().zip(&self.layer_ids) {
			path.push(*layer_id);
			layer.intersects_quad(quad, path, intersections);
			path.pop();
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		self.layers
			.iter()
			.filter_map(|layer| layer.data.bounding_box(transform * layer.transform))
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
	}
}

impl Folder {
	/// When a insertion id is provided, try to insert the layer with the given id.
	/// If that id is already used, return `None`.
	/// When no insertion id is provided, search for the next free id and insert it with that.
	/// Negative values for `insert_index` represent distance from the end
    ///
    /// # Example
    /// ```
    /// # use graphite_graphene::layers::simple_shape::Shape;
    /// # use graphite_graphene::layers::folder::Folder;
    /// # use graphite_graphene::layers::style::PathStyle;
    /// # use graphite_graphene::layers::layer_info::LayerDataType;
    /// let mut folder = Folder::default();
    ///
    /// // Create two layers to be added to the folder
    /// let mut shape_layer = Shape::rectangle(PathStyle::default());
    /// let mut folder_layer = Folder::default();
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

	/// Remove a layer with a given id from the folder.
    /// This operation will fail if `id` is not present in the folder.
    ///
    /// # Example
    /// ```
    /// # use graphite_graphene::layers::folder::Folder;
    /// let mut folder = Folder::default();
    ///
    /// // Try to remove a layer that does not exist
    /// assert!(folder.remove_layer(123).is_err());
    ///
    /// // Add another folder to the folder
    /// folder.add_layer(Folder::default().into(), Some(123), -1);
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

	/// Returns a list of layer id's in the folder.
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

    /// Get references to all the layers in the folder.
	pub fn layers(&self) -> &[Layer] {
		self.layers.as_slice()
	}

    /// Get mutable references to all the layers in the folder.
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

    /// Returns `true` if the folder contains a layer with the given id.
    ///
    /// # Example
    /// ```
    /// # use graphite_graphene::layers::folder::Folder;
    /// let mut folder = Folder::default();
    ///
    /// // Search for an id that does not exist
    /// assert!(!folder.folder_contains(123));
    ///
    /// // Add layer with the id "123" to the folder
    /// folder.add_layer(Folder::default().into(), Some(123), -1);
    ///
    /// // Search for the id "123"
    /// assert!(folder.folder_contains(123));
    /// ```
	pub fn folder_contains(&self, id: LayerId) -> bool {
		self.layer_ids.contains(&id)
	}

	/// Try to find the index of a layer with the given id within the folder.
    /// This operation will fail if no layer with the id `id` is present in the folder.
    ///
    /// # Example
    /// ```
    /// # use graphite_graphene::layers::folder::Folder;
    /// let mut folder = Folder::default();
    ///
    /// // Search for an id that does not exist
    /// assert!(folder.position_of_layer(123).is_err());
    ///
    /// // Add layer with the id "123" to the folder
    /// folder.add_layer(Folder::default().into(), Some(123), -1);
    /// folder.add_layer(Folder::default().into(), Some(42), -1);
    ///
    /// assert_eq!(folder.position_of_layer(123), Ok(0));
    /// assert_eq!(folder.position_of_layer(42), Ok(1));
    /// ```
	pub fn position_of_layer(&self, layer_id: LayerId) -> Result<usize, DocumentError> {
		self.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound([layer_id].into()))
	}

    /// Try to get a reference to a folder with the given `id`.
    /// This operation will return `None` if either no layer with `id` exists
    /// in the folder or the layer with matching id is not a folder.
    ///
    /// # Example
    /// ```
    /// # use graphite_graphene::layers::folder::Folder;
    /// # use graphite_graphene::layers::simple_shape::Shape;
    /// # use graphite_graphene::layers::style::PathStyle;
    /// let mut folder = Folder::default();
    ///
    /// // Search for an id that does not exist
    /// assert!(folder.folder(132).is_none());
    ///
    /// // add a folder and search for it
    /// folder.add_layer(Folder::default().into(), Some(123), -1);
    /// assert!(folder.folder(123).is_some());
    ///
    /// // add a non-folder layer and search for it
    /// folder.add_layer(Shape::rectangle(PathStyle::default()).into(), Some(42), -1);
    /// assert!(folder.folder(42).is_none());
    /// ```
	pub fn folder(&self, id: LayerId) -> Option<&Folder> {
		match self.layer(id) {
			Some(Layer {
				data: LayerDataType::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}

    /// Try to get a mutable reference to folder with the given `id`.
    /// This operation will return `None` if either no layer with `id` exists
    /// in the folder or the layer with matching id is not a folder.
    /// See the [Folder::folder] method for a usage example.
	pub fn folder_mut(&mut self, id: LayerId) -> Option<&mut Folder> {
		match self.layer_mut(id) {
			Some(Layer {
				data: LayerDataType::Folder(folder), ..
			}) => Some(folder),
			_ => None,
		}
	}
}
