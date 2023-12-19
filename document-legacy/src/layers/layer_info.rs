use super::folder_layer::FolderLegacyLayer;
use super::layer_layer::LayerLegacyLayer;
use super::style::RenderData;
use crate::intersection::Quad;
use crate::DocumentError;
use crate::LayerId;

use graphene_core::vector::VectorData;

use core::fmt;
use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};

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

// ===========
// LegacyLayer
// ===========

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
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

	/// Gets a child layer of this layer, by a path. If the layer with id 1 is inside a folder with id 0, the path will be [0, 1].
	pub fn child(&self, path: &[LayerId]) -> Option<&LegacyLayer> {
		let mut layer = self;
		for id in path {
			layer = layer.as_folder().ok()?.layer(*id)?;
		}
		Some(layer)
	}

	/// Gets a child layer of this layer, by a path. If the layer with id 1 is inside a folder with id 0, the path will be [0, 1].
	pub fn child_mut(&mut self, path: &[LayerId]) -> Option<&mut LegacyLayer> {
		let mut layer = self;
		for id in path {
			layer = layer.as_folder_mut().ok()?.layer_mut(*id)?;
		}
		Some(layer)
	}

	/// Iterate over the layers encapsulated by this layer.
	/// If the [Layer type](Layer::data) is not a folder, the only item in the iterator will be the layer itself.
	/// If the [Layer type](Layer::data) wraps a [Folder](LegacyLayerType::Folder), the iterator will recursively yield all the layers contained in the folder as well as potential sub-folders.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::layer_info::Layer;
	/// # use graphite_document_legacy::layers::style::PathStyle;
	/// # use graphite_document_legacy::layers::folder_layer::FolderLegacyLayer;
	/// let mut root_folder = FolderLegacyLayer::default();
	///
	/// // Add a shape to the root folder
	/// let child_1: Layer = ShapeLegacyLayer::rectangle(PathStyle::default()).into();
	/// root_folder.add_layer(child_1.clone(), None, -1);
	///
	/// // Add a folder containing another shape to the root layer
	/// let mut child_folder = FolderLegacyLayer::default();
	/// let grandchild: Layer = ShapeLegacyLayer::rectangle(PathStyle::default()).into();
	/// child_folder.add_layer(grandchild.clone(), None, -1);
	/// let child_2: Layer = child_folder.into();
	/// root_folder.add_layer(child_2.clone(), None, -1);
	/// let root: Layer = root_folder.into();
	///
	/// let mut iter = root.iter();
	/// assert_eq!(iter.next(), Some(&root));
	/// assert_eq!(iter.next(), Some(&child_2));
	/// assert_eq!(iter.next(), Some(&grandchild));
	/// assert_eq!(iter.next(), Some(&child_1));
	/// assert_eq!(iter.next(), None);
	/// ```
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

	/// Compute the bounding box of the layer after applying a transform to it.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::layer_info::Layer;
	/// # use graphite_document_legacy::layers::style::{PathStyle, RenderData};
	/// # use glam::DVec2;
	/// # use glam::f64::DAffine2;
	/// # use std::collections::HashMap;
	/// // Create a rectangle with the default dimensions, from `(0|0)` to `(1|1)`
	/// let layer: Layer = ShapeLegacyLayer::rectangle(PathStyle::default()).into();
	///
	/// // Apply the Identity transform, which leaves the points unchanged
	/// let transform = DAffine2::IDENTITY;
	/// let font_cache = Default::default();
	/// let render_data = RenderData::new(&font_cache, Default::default(), None);
	/// assert_eq!(
	///     layer.aabb_for_transform(transform, &render_data),
	///     Some([DVec2::ZERO, DVec2::ONE]),
	/// );
	///
	/// // Apply a transform that scales every point by a factor of two
	/// let transform = DAffine2::from_scale(DVec2::ONE * 2.);
	/// assert_eq!(
	///     layer.aabb_for_transform(transform, &render_data),
	///     Some([DVec2::ZERO, DVec2::ONE * 2.]),
	/// );
	pub fn aabb_for_transform(&self, transform: DAffine2, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.data.bounding_box(transform, render_data)
	}

	pub fn aabb(&self, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.aabb_for_transform(self.transform, render_data)
	}

	pub fn bounding_transform(&self, render_data: &RenderData) -> DAffine2 {
		let scale = match self.aabb_for_transform(DAffine2::IDENTITY, render_data) {
			Some([a, b]) => {
				let dimensions = b - a;
				DAffine2::from_scale(dimensions)
			}
			None => DAffine2::IDENTITY,
		};

		self.transform * scale
	}

	/// Get a mutable reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Folder`.
	pub fn as_folder_mut(&mut self) -> Result<&mut FolderLegacyLayer, DocumentError> {
		match &mut self.data {
			LegacyLayerType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotFolder),
		}
	}

	pub fn as_vector_data(&self) -> Option<&VectorData> {
		match &self.data {
			LegacyLayerType::Layer(layer) => layer.as_vector_data(),
			_ => None,
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

	/// Get a mutable reference to the NodeNetwork
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Layer`.
	pub fn as_layer_network_mut(&mut self) -> Result<&mut graph_craft::document::NodeNetwork, DocumentError> {
		match &mut self.data {
			LegacyLayerType::Layer(layer) => Ok(&mut layer.network),
			_ => Err(DocumentError::NotLayer),
		}
	}

	/// Get a reference to the NodeNetwork
	/// This operation will fail if the [Layer type](Layer::data) is not `LegacyLayerType::Layer`.
	pub fn as_layer_network(&self) -> Result<&graph_craft::document::NodeNetwork, DocumentError> {
		match &self.data {
			LegacyLayerType::Layer(layer) => Ok(&layer.network),
			_ => Err(DocumentError::NotLayer),
		}
	}

	pub fn as_layer(&self) -> Result<&LayerLegacyLayer, DocumentError> {
		match &self.data {
			LegacyLayerType::Layer(layer) => Ok(layer),
			_ => Err(DocumentError::NotLayer),
		}
	}
}

impl Clone for LegacyLayer {
	fn clone(&self) -> Self {
		Self {
			visible: self.visible,
			name: self.name.clone(),
			data: self.data.clone(),
			transform: self.transform,
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

#[derive(Serialize, Deserialize)]
#[serde(remote = "glam::DAffine2")]
struct DAffine2Ref {
	pub matrix2: DMat2,
	pub translation: DVec2,
}
