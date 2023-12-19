use super::layer_info::{LayerData, LegacyLayer, LegacyLayerType};
use super::style::RenderData;
use crate::intersection::Quad;
use crate::{DocumentError, LayerId};

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
	/// Returns a list of [LayerId]s in the folder.
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	/// Get references to all the [Layer]s in the folder.
	pub fn layers(&self) -> &[LegacyLayer] {
		self.layers.as_slice()
	}

	pub fn layer(&self, id: LayerId) -> Option<&LegacyLayer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&self.layers[pos])
	}

	pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut LegacyLayer> {
		let pos = self.position_of_layer(id).ok()?;
		Some(&mut self.layers[pos])
	}

	/// Tries to find the index of a layer with the given [LayerId] within the folder.
	/// This operation will fail if no layer with a matching ID is present in the folder.
	pub fn position_of_layer(&self, layer_id: LayerId) -> Result<usize, DocumentError> {
		self.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound([layer_id].into()))
	}

	/// Tries to get a reference to a folder with the given [LayerId].
	/// This operation will return `None` if either no layer with `id` exists
	/// in the folder, or the layer with matching ID is not a folder.
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
