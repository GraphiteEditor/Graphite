use super::folder_layer::FolderLegacyLayer;
use super::layer_layer::LayerLegacyLayer;
use super::style::RenderData;
use crate::intersection::Quad;
use crate::DocumentError;
use crate::LayerId;

use core::fmt;
use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};

// =========
// LayerData
// =========

/// Defines shared behavior for every layer type.
pub trait LayerData {
	/// Render the layer as an SVG tag to a given string, returning a boolean to indicate if a redraw is required next frame.
	fn render(&mut self, cache_inner_svg: &mut String, cache_defs_svg: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: &RenderData) -> bool;

	/// Determine the layers within this layer that intersect a given quad.
	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData);

	/// Calculate the bounding box for the layer's contents after applying a given transform.
	fn bounding_box(&self, transform: glam::DAffine2, render_data: &RenderData) -> Option<[DVec2; 2]>;
}

// ===============
// LegacyLayerType
// ===============

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Represents different types of layers.
pub enum LegacyLayerType {
	/// A layer that wraps a [FolderLegacyLayer] struct.
	Folder(FolderLegacyLayer),
	/// A layer that wraps an [LayerLegacyLayer] struct.
	Layer(LayerLegacyLayer),
}

impl Default for LegacyLayerType {
	fn default() -> Self {
		LegacyLayerType::Layer(Default::default())
	}
}

impl LegacyLayerType {
	pub fn inner(&self) -> &dyn LayerData {
		match self {
			LegacyLayerType::Folder(folder) => folder,
			LegacyLayerType::Layer(layer) => layer,
		}
	}

	pub fn inner_mut(&mut self) -> &mut dyn LayerData {
		match self {
			LegacyLayerType::Folder(folder) => folder,
			LegacyLayerType::Layer(layer) => layer,
		}
	}
}

impl LayerData for LegacyLayerType {
	fn render(&mut self, cache_inner_svg: &mut String, cache_defs_svg: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: &RenderData) -> bool {
		self.inner_mut()/*called_via_thumbnail*/.render(cache_inner_svg, cache_defs_svg, transforms, render_data)
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData) {
		self.inner().intersects_quad(quad, path, intersections, render_data)
	}

	fn bounding_box(&self, transform: glam::DAffine2, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.inner().bounding_box(transform, render_data)
	}
}

// =========================
// LayerDataTypeDiscriminant
// =========================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, specta::Type)]
pub enum LayerDataTypeDiscriminant {
	Folder,
	Layer,
}

impl fmt::Display for LayerDataTypeDiscriminant {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			LayerDataTypeDiscriminant::Folder => write!(f, "Folder"),
			LayerDataTypeDiscriminant::Layer => write!(f, "Layer"),
		}
	}
}

impl From<&LegacyLayerType> for LayerDataTypeDiscriminant {
	fn from(data: &LegacyLayerType) -> Self {
		use LegacyLayerType::*;

		match data {
			Folder(_) => LayerDataTypeDiscriminant::Folder,
			Layer(_) => LayerDataTypeDiscriminant::Layer,
		}
	}
}

// ===========
// LegacyLayer
// ===========

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct LegacyLayer {
	/// Whether the layer is currently visible or hidden.
	pub visible: bool,
	/// The user-given name of the layer.
	pub name: Option<String>,
	/// The type of layer, such as folder or shape.
	pub data: LegacyLayerType,
	/// A transformation applied to the layer (translation, rotation, scaling, and shear).
	#[serde(with = "DAffine2Ref")]
	pub transform: glam::DAffine2,
}

impl LegacyLayer {
	pub fn new(data: LegacyLayerType, transform: [f64; 6]) -> Self {
		Self {
			visible: true,
			name: None,
			data,
			transform: glam::DAffine2::from_cols_array(&transform),
		}
	}

	/// Iterate over the layers encapsulated by this layer.
	/// If the [Layer type](Layer::data) is not a folder, the only item in the iterator will be the layer itself.
	/// If the [Layer type](Layer::data) wraps a [Folder](LegacyLayerType::Folder), the iterator will recursively yield all the layers contained in the folder as well as potential sub-folders.
	pub fn iter(&self) -> LayerIter<'_> {
		LayerIter { stack: vec![self] }
	}

	pub fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData) {
		if !self.visible {
			return;
		}

		let transformed_quad = self.transform.inverse() * quad;
		self.data.intersects_quad(transformed_quad, path, intersections, render_data)
	}

	/// Get a mutable reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder_mut(&mut self) -> Result<&mut FolderLegacyLayer, DocumentError> {
		match &mut self.data {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
	}

	/// Get a reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder(&self) -> Result<&FolderLegacyLayer, DocumentError> {
		match &self.data {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
	}
}

impl From<FolderLegacyLayer> for LegacyLayer {
	fn from(from: FolderLegacyLayer) -> LegacyLayer {
		LegacyLayer::new(LegacyLayerType::Folder(from), DAffine2::IDENTITY.to_cols_array())
	}
}

impl<'a> IntoIterator for &'a LegacyLayer {
	type Item = &'a LegacyLayer;
	type IntoIter = LayerIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

/// An iterator over the layers encapsulated by this layer.
/// See [Layer::iter] for more information.
#[derive(Debug, Default)]
pub struct LayerIter<'a> {
	pub stack: Vec<&'a LegacyLayer>,
}

impl<'a> Iterator for LayerIter<'a> {
	type Item = &'a LegacyLayer;

	fn next(&mut self) -> Option<Self::Item> {
		match self.stack.pop() {
			Some(layer) => {
				if let LegacyLayerType::Folder(folder) = &layer.data {
					let layers = folder.layers();
					self.stack.extend(layers);
				};
				Some(layer)
			}
			None => None,
		}
	}
}

// ===========
// DAffine2Ref
// ===========

#[derive(Serialize, Deserialize)]
#[serde(remote = "glam::DAffine2")]
struct DAffine2Ref {
	pub matrix2: DMat2,
	pub translation: DVec2,
}
