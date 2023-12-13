use super::folder_layer::FolderLegacyLayer;
use super::layer_layer::LayerLegacyLayer;
use super::shape_layer::ShapeLegacyLayer;
use super::style::{PathStyle, RenderData};
use crate::intersection::Quad;
use crate::DocumentError;
use crate::LayerId;

use graphene_core::raster::BlendMode;
use graphene_core::vector::VectorData;
use graphene_std::vector::subpath::Subpath;

use core::fmt;
use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Represents different types of layers.
pub enum LegacyLayerType {
	/// A layer that wraps a [FolderLegacyLayer] struct.
	Folder(FolderLegacyLayer),
	/// A layer that wraps a [ShapeLegacyLayer] struct. Still used by the overlays system, but will be removed in the future.
	Shape(ShapeLegacyLayer),
	/// A layer that wraps an [LayerLegacyLayer] struct.
	Layer(LayerLegacyLayer),
}

impl Default for LegacyLayerType {
	fn default() -> Self {
		LegacyLayerType::Folder(FolderLegacyLayer::default())
	}
}

impl LegacyLayerType {
	pub fn inner(&self) -> &dyn LayerData {
		match self {
			LegacyLayerType::Shape(shape) => shape,
			LegacyLayerType::Folder(folder) => folder,
			LegacyLayerType::Layer(layer) => layer,
		}
	}

	pub fn inner_mut(&mut self) -> &mut dyn LayerData {
		match self {
			LegacyLayerType::Shape(shape) => shape,
			LegacyLayerType::Folder(folder) => folder,
			LegacyLayerType::Layer(layer) => layer,
		}
	}
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, specta::Type)]
pub enum LayerDataTypeDiscriminant {
	Folder,
	Shape,
	Layer,
	Artboard,
}

impl fmt::Display for LayerDataTypeDiscriminant {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			LayerDataTypeDiscriminant::Folder => write!(f, "Folder"),
			LayerDataTypeDiscriminant::Shape => write!(f, "Shape"),
			LayerDataTypeDiscriminant::Layer => write!(f, "Layer"),
			LayerDataTypeDiscriminant::Artboard => write!(f, "Artboard"),
		}
	}
}

impl From<&LegacyLayerType> for LayerDataTypeDiscriminant {
	fn from(data: &LegacyLayerType) -> Self {
		use LegacyLayerType::*;

		match data {
			Folder(_) => LayerDataTypeDiscriminant::Folder,
			Shape(_) => LayerDataTypeDiscriminant::Shape,
			Layer(_) => LayerDataTypeDiscriminant::Layer,
		}
	}
}

// ** CONVERSIONS **

impl<'a> TryFrom<&'a mut LegacyLayer> for &'a mut Subpath {
	type Error = &'static str;
	/// Convert a mutable layer into a mutable [Subpath].
	fn try_from(layer: &'a mut LegacyLayer) -> Result<&'a mut Subpath, Self::Error> {
		match &mut layer.data {
			LegacyLayerType::Shape(layer) => Ok(&mut layer.shape),
			_ => Err("Did not find any shape data in the layer"),
		}
	}
}

impl<'a> TryFrom<&'a LegacyLayer> for &'a Subpath {
	type Error = &'static str;
	/// Convert a reference to a layer into a reference of a [Subpath].
	fn try_from(layer: &'a LegacyLayer) -> Result<&'a Subpath, Self::Error> {
		match &layer.data {
			LegacyLayerType::Shape(layer) => Ok(&layer.shape),
			_ => Err("Did not find any shape data in the layer"),
		}
	}
}

/// Defines shared behavior for every layer type.
pub trait LayerData {
	/// Render the layer as an SVG tag to a given string, returning a boolean to indicate if a redraw is required next frame.
	///
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::style::{Fill, PathStyle, ViewMode, RenderData};
	/// # use graphite_document_legacy::layers::layer_info::LayerData;
	/// # use std::collections::HashMap;
	///
	/// let mut shape = ShapeLegacyLayer::rectangle(PathStyle::new(None, Fill::None));
	/// let mut svg = String::new();
	///
	/// // Render the shape without any transforms, in normal view mode
	/// # let font_cache = Default::default();
	/// let render_data = RenderData::new(&font_cache, ViewMode::Normal, None);
	/// shape.render(&mut svg, &mut String::new(), &mut vec![], &render_data);
	///
	/// assert_eq!(
	///     svg,
	///     "<g transform=\"matrix(\n1,-0,-0,1,-0,-0)\">\
	///     <path d=\"M0,0L0,1L1,1L1,0Z\"  fill=\"none\" />\
	///     </g>"
	/// );
	/// ```
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: &RenderData) -> bool;

	/// Determine the layers within this layer that intersect a given quad.
	/// # Example
	/// ```
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::style::{Fill, PathStyle, ViewMode, RenderData};
	/// # use graphite_document_legacy::layers::layer_info::LayerData;
	/// # use graphite_document_legacy::intersection::Quad;
	/// # use glam::f64::{DAffine2, DVec2};
	/// # use std::collections::HashMap;
	///
	/// let mut shape = ShapeLegacyLayer::ellipse(PathStyle::new(None, Fill::None));
	/// let shape_id = 42;
	/// let mut svg = String::new();
	///
	/// let quad = Quad::from_box([DVec2::ZERO, DVec2::ONE]);
	/// let mut intersections = vec![];
	///
	/// let font_cache = Default::default();
	/// let render_data = RenderData::new(&font_cache, Default::default(), None);
	/// shape.intersects_quad(quad, &mut vec![shape_id], &mut intersections, &render_data);
	///
	/// assert_eq!(intersections, vec![vec![shape_id]]);
	/// ```
	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData);

	// TODO: this doctest fails because 0 != 1e-32, maybe assert difference < epsilon?
	/// Calculate the bounding box for the layer's contents after applying a given transform.
	/// # Example
	/// ```no_run
	/// # use graphite_document_legacy::layers::shape_layer::ShapeLegacyLayer;
	/// # use graphite_document_legacy::layers::style::{Fill, PathStyle, RenderData};
	/// # use graphite_document_legacy::layers::layer_info::LayerData;
	/// # use glam::f64::{DAffine2, DVec2};
	/// # use std::collections::HashMap;
	/// let shape = ShapeLegacyLayer::ellipse(PathStyle::new(None, Fill::None));
	///
	/// // Calculate the bounding box without applying any transformations.
	/// // (The identity transform maps every vector to itself.)
	/// let transform = DAffine2::IDENTITY;
	/// let font_cache = Default::default();
	/// let render_data = RenderData::new(&font_cache, Default::default(), None);
	/// let bounding_box = shape.bounding_box(transform, &render_data);
	///
	/// assert_eq!(bounding_box, Some([DVec2::ZERO, DVec2::ONE]));
	/// ```
	fn bounding_box(&self, transform: glam::DAffine2, render_data: &RenderData) -> Option<[DVec2; 2]>;
}

impl LayerData for LegacyLayerType {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: &RenderData) -> bool {
		self.inner_mut().render(svg, svg_defs, transforms, render_data)
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, render_data: &RenderData) {
		self.inner().intersects_quad(quad, path, intersections, render_data)
	}

	fn bounding_box(&self, transform: glam::DAffine2, render_data: &RenderData) -> Option<[DVec2; 2]> {
		self.inner().bounding_box(transform, render_data)
	}
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "glam::DAffine2")]
struct DAffine2Ref {
	pub matrix2: DMat2,
	pub translation: DVec2,
}

/// Utility function for providing a default boolean value to serde.
#[inline(always)]
fn return_true() -> bool {
	true
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
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
	/// Should the aspect ratio of this layer be preserved?
	#[serde(default = "return_true")]
	pub preserve_aspect: bool,
	/// The center of transformations like rotation or scaling with the shift key.
	/// This is in local space (so the layer's transform should be applied).
	pub pivot: DVec2,
	/// The cached SVG thumbnail view of the layer.
	#[serde(skip)]
	pub thumbnail_cache: String,
	/// The cached SVG render of the layer.
	#[serde(skip)]
	pub cache: String,
	/// The cached definition(s) used by the layer's SVG tag, placed at the top in the SVG defs tag.
	#[serde(skip)]
	pub svg_defs_cache: String,
	/// Whether or not the [Cache](Layer::cache) and [Thumbnail Cache](Layer::thumbnail_cache) need to be updated.
	#[serde(skip, default = "return_true")]
	pub cache_dirty: bool,
	/// The blend mode describing how this layer should composite with others underneath it.
	pub blend_mode: BlendMode,
	/// The opacity, in the range of 0 to 1.
	pub opacity: f64,
}

impl Default for LegacyLayer {
	fn default() -> Self {
		Self {
			visible: Default::default(),
			name: Default::default(),
			data: Default::default(),
			transform: Default::default(),
			preserve_aspect: Default::default(),
			pivot: Default::default(),
			thumbnail_cache: Default::default(),
			cache: Default::default(),
			svg_defs_cache: Default::default(),
			cache_dirty: Default::default(),
			blend_mode: Default::default(),
			opacity: Default::default(),
		}
	}
}

impl LegacyLayer {
	pub fn new(data: LegacyLayerType, transform: [f64; 6]) -> Self {
		Self {
			visible: true,
			name: None,
			data,
			transform: glam::DAffine2::from_cols_array(&transform),
			preserve_aspect: true,
			pivot: DVec2::splat(0.5),
			cache: String::new(),
			thumbnail_cache: String::new(),
			svg_defs_cache: String::new(),
			cache_dirty: true,
			blend_mode: BlendMode::Normal,
			opacity: 1.,
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

	/// Renders the layer, returning the result and if a redraw is required
	pub fn render(&mut self, transforms: &mut Vec<DAffine2>, svg_defs: &mut String, render_data: &RenderData) -> (&str, bool) {
		if !self.visible {
			return ("", false);
		}

		transforms.push(self.transform);

		// Skip rendering if outside the viewport bounds
		if let Some(viewport_bounds) = render_data.culling_bounds {
			if let Some(bounding_box) = self.data.bounding_box(transforms.iter().cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY), render_data) {
				let is_overlapping =
					viewport_bounds[0].x < bounding_box[1].x && bounding_box[0].x < viewport_bounds[1].x && viewport_bounds[0].y < bounding_box[1].y && bounding_box[0].y < viewport_bounds[1].y;
				if !is_overlapping {
					transforms.pop();
					self.cache.clear();
					self.cache_dirty = true;
					return ("", true);
				}
			}
		}

		let mut requires_redraw = false;

		if self.cache_dirty {
			self.thumbnail_cache.clear();
			self.svg_defs_cache.clear();
			requires_redraw = self.data.render(&mut self.thumbnail_cache, &mut self.svg_defs_cache, transforms, render_data);

			self.cache.clear();
			let _ = writeln!(self.cache, r#"<g transform="matrix("#);
			self.transform.to_cols_array().iter().enumerate().for_each(|(i, f)| {
				let _ = self.cache.write_str(&(f.to_string() + if i == 5 { "" } else { "," }));
			});
			let _ = write!(self.cache, r#")" style="opacity: {};{}">{}</g>"#, self.opacity, self.blend_mode.render(), self.thumbnail_cache.as_str());

			self.cache_dirty = false;
		}

		transforms.pop();
		svg_defs.push_str(&self.svg_defs_cache);

		// If a redraw is required then set the cache to dirty.
		if requires_redraw {
			self.cache_dirty = true;
		}

		(self.cache.as_str(), requires_redraw)
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

	pub fn layerspace_pivot(&self, render_data: &RenderData) -> DVec2 {
		let [mut min, max] = self.aabb_for_transform(DAffine2::IDENTITY, render_data).unwrap_or([DVec2::ZERO, DVec2::ONE]);

		// If the layer bounds are 0 in either axis then set them to one (to avoid div 0)
		if (max.x - min.x) < f64::EPSILON * 1000. {
			min.x = max.x - 1.;
		}
		if (max.y - min.y) < f64::EPSILON * 1000. {
			min.y = max.y - 1.;
		}

		self.pivot * (max - min) + min
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

	pub fn as_subpath_mut(&mut self) -> Option<&mut Subpath> {
		match &mut self.data {
			LegacyLayerType::Shape(s) => Some(&mut s.shape),
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

	pub fn style(&self) -> Result<&PathStyle, DocumentError> {
		match &self.data {
			LegacyLayerType::Shape(shape) => Ok(&shape.style),
			LegacyLayerType::Layer(layer) => layer.as_vector_data().map(|vector| &vector.style).ok_or(DocumentError::NotShape),
			_ => Err(DocumentError::NotShape),
		}
	}

	pub fn style_mut(&mut self) -> Result<&mut PathStyle, DocumentError> {
		match &mut self.data {
			LegacyLayerType::Shape(s) => Ok(&mut s.style),
			_ => Err(DocumentError::NotShape),
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
			preserve_aspect: self.preserve_aspect,
			pivot: self.pivot,
			cache: String::new(),
			thumbnail_cache: String::new(),
			svg_defs_cache: String::new(),
			cache_dirty: true,
			blend_mode: self.blend_mode,
			opacity: self.opacity,
		}
	}
}

impl From<FolderLegacyLayer> for LegacyLayer {
	fn from(from: FolderLegacyLayer) -> LegacyLayer {
		LegacyLayer::new(LegacyLayerType::Folder(from), DAffine2::IDENTITY.to_cols_array())
	}
}

impl From<ShapeLegacyLayer> for LegacyLayer {
	fn from(from: ShapeLegacyLayer) -> LegacyLayer {
		LegacyLayer::new(LegacyLayerType::Shape(from), DAffine2::IDENTITY.to_cols_array())
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
