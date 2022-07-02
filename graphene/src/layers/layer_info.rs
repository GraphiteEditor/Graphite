use super::blend_mode::BlendMode;
use super::folder_layer::FolderLayer;
use super::image_layer::ImageLayer;
use super::shape_layer::ShapeLayer;
use super::style::{PathStyle, RenderData};
use super::text_layer::TextLayer;
use super::vector::vector_shape::VectorShape;
use crate::intersection::Quad;
use crate::layers::text_layer::FontCache;
use crate::DocumentError;
use crate::LayerId;

use core::fmt;
use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Represents different types of layers.
pub enum LayerDataType {
	/// A layer that wraps a [FolderLayer] struct.
	Folder(FolderLayer),
	/// A layer that wraps a [ShapeLayer] struct.
	Shape(ShapeLayer),
	/// A layer that wraps a [TextLayer] struct.
	Text(TextLayer),
	/// A layer that wraps an [ImageLayer] struct.
	Image(ImageLayer),
}

impl LayerDataType {
	pub fn inner(&self) -> &dyn LayerData {
		match self {
			LayerDataType::Shape(s) => s,
			LayerDataType::Folder(f) => f,
			LayerDataType::Text(t) => t,
			LayerDataType::Image(i) => i,
		}
	}

	pub fn inner_mut(&mut self) -> &mut dyn LayerData {
		match self {
			LayerDataType::Shape(s) => s,
			LayerDataType::Folder(f) => f,
			LayerDataType::Text(t) => t,
			LayerDataType::Image(i) => i,
		}
	}
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LayerDataTypeDiscriminant {
	Folder,
	Shape,
	Text,
	Image,
}

impl fmt::Display for LayerDataTypeDiscriminant {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let name = match self {
			LayerDataTypeDiscriminant::Folder => "Folder",
			LayerDataTypeDiscriminant::Shape => "Shape",
			LayerDataTypeDiscriminant::Text => "Text",
			LayerDataTypeDiscriminant::Image => "Image",
		};

		formatter.write_str(name)
	}
}

impl From<&LayerDataType> for LayerDataTypeDiscriminant {
	fn from(data: &LayerDataType) -> Self {
		use LayerDataType::*;

		match data {
			Folder(_) => LayerDataTypeDiscriminant::Folder,
			Shape(_) => LayerDataTypeDiscriminant::Shape,
			Text(_) => LayerDataTypeDiscriminant::Text,
			Image(_) => LayerDataTypeDiscriminant::Image,
		}
	}
}

/// Defines shared behavior for every layer type.
pub trait LayerData {
	/// Render the layer as an SVG tag to a given string.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::style::{Fill, PathStyle, ViewMode, RenderData};
	/// # use graphite_graphene::layers::layer_info::LayerData;
	/// # use std::collections::HashMap;
	///
	/// let mut shape = ShapeLayer::rectangle(PathStyle::new(None, Fill::None));
	/// let mut svg = String::new();
	///
	/// // Render the shape without any transforms, in normal view mode
	/// # let font_cache = Default::default();
	/// let render_data = RenderData::new(ViewMode::Normal, &font_cache, None, false);
	/// shape.render(&mut svg, &mut String::new(), &mut vec![], render_data);
	///
	/// assert_eq!(
	///     svg,
	///     "<g transform=\"matrix(\n1,-0,-0,1,-0,-0)\">\
	///     <path d=\"M0,0L0,1L1,1L1,0Z\"  fill=\"none\" />\
	///     </g>"
	/// );
	/// ```
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: RenderData);

	/// Determine the layers within this layer that intersect a given quad.
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::style::{Fill, PathStyle, ViewMode};
	/// # use graphite_graphene::layers::layer_info::LayerData;
	/// # use graphite_graphene::intersection::Quad;
	/// # use glam::f64::{DAffine2, DVec2};
	/// # use std::collections::HashMap;
	///
	/// let mut shape = ShapeLayer::ellipse(PathStyle::new(None, Fill::None));
	/// let shape_id = 42;
	/// let mut svg = String::new();
	///
	/// let quad = Quad::from_box([DVec2::ZERO, DVec2::ONE*2.]);
	/// let mut intersections = vec![];
	///
	/// shape.intersects_quad(quad, &mut vec![shape_id], &mut intersections, &Default::default());
	///
	/// assert_eq!(intersections, vec![vec![shape_id]]);
	/// ```
	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, font_cache: &FontCache);

	// TODO: this doctest fails because 0 != 1e-32, maybe assert difference < epsilon?
	/// Calculate the bounding box for the layer's contents after applying a given transform.
	/// # Example
	/// ```no_run
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::style::{Fill, PathStyle};
	/// # use graphite_graphene::layers::layer_info::LayerData;
	/// # use glam::f64::{DAffine2, DVec2};
	/// # use std::collections::HashMap;
	/// let shape = ShapeLayer::ellipse(PathStyle::new(None, Fill::None));
	///
	/// // Calculate the bounding box without applying any transformations.
	/// // (The identity transform maps every vector to itself.)
	/// let transform = DAffine2::IDENTITY;
	/// let bounding_box = shape.bounding_box(transform, &Default::default());
	///
	/// assert_eq!(bounding_box, Some([DVec2::ZERO, DVec2::ONE]));
	/// ```
	fn bounding_box(&self, transform: glam::DAffine2, font_cache: &FontCache) -> Option<[DVec2; 2]>;
}

impl LayerData for LayerDataType {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<glam::DAffine2>, render_data: RenderData) {
		self.inner_mut().render(svg, svg_defs, transforms, render_data)
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, font_cache: &FontCache) {
		self.inner().intersects_quad(quad, path, intersections, font_cache)
	}

	fn bounding_box(&self, transform: glam::DAffine2, font_cache: &FontCache) -> Option<[DVec2; 2]> {
		self.inner().bounding_box(transform, font_cache)
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
pub struct Layer {
	/// Whether the layer is currently visible or hidden.
	pub visible: bool,
	/// The user-given name of the layer.
	pub name: Option<String>,
	/// The type of layer, such as folder or shape.
	pub data: LayerDataType,
	/// A transformation applied to the layer (translation, rotation, scaling, and shear).
	#[serde(with = "DAffine2Ref")]
	pub transform: glam::DAffine2,
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

impl Layer {
	pub fn new(data: LayerDataType, transform: [f64; 6]) -> Self {
		Self {
			visible: true,
			name: None,
			data,
			transform: glam::DAffine2::from_cols_array(&transform),
			cache: String::new(),
			thumbnail_cache: String::new(),
			svg_defs_cache: String::new(),
			cache_dirty: true,
			blend_mode: BlendMode::Normal,
			opacity: 1.,
		}
	}

	/// Iterate over the layers encapsulated by this layer.
	/// If the [Layer type](Layer::data) is not a folder, the only item in the iterator will be the layer itself.
	/// If the [Layer type](Layer::data) wraps a [Folder](LayerDataType::Folder), the iterator will recursively yield all the layers contained in the folder as well as potential sub-folders.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::layer_info::Layer;
	/// # use graphite_graphene::layers::style::PathStyle;
	/// # use graphite_graphene::layers::folder_layer::FolderLayer;
	/// let mut root_folder = FolderLayer::default();
	///
	/// // Add a shape to the root folder
	/// let child_1: Layer = ShapeLayer::rectangle(PathStyle::default()).into();
	/// root_folder.add_layer(child_1.clone(), None, -1);
	///
	/// // Add a folder containing another shape to the root layer
	/// let mut child_folder = FolderLayer::default();
	/// let grandchild: Layer = ShapeLayer::rectangle(PathStyle::default()).into();
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

	pub fn render(&mut self, transforms: &mut Vec<DAffine2>, svg_defs: &mut String, render_data: RenderData) -> &str {
		if !self.visible {
			return "";
		}

		transforms.push(self.transform);
		if let Some(viewport_bounds) = render_data.culling_bounds {
			if let Some(bounding_box) = self
				.data
				.bounding_box(transforms.iter().cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY), render_data.font_cache)
			{
				let is_overlapping =
					viewport_bounds[0].x < bounding_box[1].x && bounding_box[0].x < viewport_bounds[1].x && viewport_bounds[0].y < bounding_box[1].y && bounding_box[0].y < viewport_bounds[1].y;
				if !is_overlapping {
					transforms.pop();
					self.cache.clear();
					self.cache_dirty = true;
					return "";
				}
			}
		}

		if self.cache_dirty {
			self.thumbnail_cache.clear();
			self.svg_defs_cache.clear();
			self.data.render(&mut self.thumbnail_cache, &mut self.svg_defs_cache, transforms, render_data);

			self.cache.clear();
			let _ = writeln!(self.cache, r#"<g transform="matrix("#);
			self.transform.to_cols_array().iter().enumerate().for_each(|(i, f)| {
				let _ = self.cache.write_str(&(f.to_string() + if i == 5 { "" } else { "," }));
			});
			let _ = write!(
				self.cache,
				r#")" style="mix-blend-mode: {}; opacity: {}">{}</g>"#,
				self.blend_mode.to_svg_style_name(),
				self.opacity,
				self.thumbnail_cache.as_str()
			);

			self.cache_dirty = false;
		}
		transforms.pop();
		svg_defs.push_str(&self.svg_defs_cache);

		self.cache.as_str()
	}

	pub fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, font_cache: &FontCache) {
		if !self.visible {
			return;
		}

		let transformed_quad = self.transform.inverse() * quad;
		self.data.intersects_quad(transformed_quad, path, intersections, font_cache)
	}

	/// Compute the bounding box of the layer after applying a transform to it.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::layer_info::Layer;
	/// # use graphite_graphene::layers::style::PathStyle;
	/// # use glam::DVec2;
	/// # use glam::f64::DAffine2;
	/// # use std::collections::HashMap;
	/// // Create a rectangle with the default dimensions, from `(0|0)` to `(1|1)`
	/// let layer: Layer = ShapeLayer::rectangle(PathStyle::default()).into();
	///
	/// // Apply the Identity transform, which leaves the points unchanged
	/// assert_eq!(
	///     layer.aabounding_box_for_transform(DAffine2::IDENTITY, &Default::default()),
	///     Some([DVec2::ZERO, DVec2::ONE]),
	/// );
	///
	/// // Apply a transform that scales every point by a factor of two
	/// let transform = DAffine2::from_scale(DVec2::ONE * 2.);
	/// assert_eq!(
	///     layer.aabounding_box_for_transform(transform, &Default::default()),
	///     Some([DVec2::ZERO, DVec2::ONE * 2.]),
	/// );
	pub fn aabounding_box_for_transform(&self, transform: DAffine2, font_cache: &FontCache) -> Option<[DVec2; 2]> {
		self.data.bounding_box(transform, font_cache)
	}

	pub fn aabounding_box(&self, font_cache: &FontCache) -> Option<[DVec2; 2]> {
		self.aabounding_box_for_transform(self.transform, font_cache)
	}

	pub fn bounding_transform(&self, font_cache: &FontCache) -> DAffine2 {
		let scale = match self.aabounding_box_for_transform(DAffine2::IDENTITY, font_cache) {
			Some([a, b]) => {
				let dimensions = b - a;
				DAffine2::from_scale(dimensions)
			}
			_ => DAffine2::IDENTITY,
		};

		self.transform * scale
	}

	/// Get a mutable reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Folder`.
	pub fn as_folder_mut(&mut self) -> Result<&mut FolderLayer, DocumentError> {
		match &mut self.data {
			LayerDataType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

	pub fn as_vector_shape(&self) -> Option<&VectorShape> {
		match &self.data {
			LayerDataType::Shape(s) => Some(&s.shape),
			_ => None,
		}
	}

	pub fn as_vector_shape_copy(&self) -> Option<VectorShape> {
		match &self.data {
			LayerDataType::Shape(s) => Some(s.shape.clone()),
			_ => None,
		}
	}

	pub fn as_vector_shape_mut(&mut self) -> Option<&mut VectorShape> {
		match &mut self.data {
			LayerDataType::Shape(s) => Some(&mut s.shape),
			_ => None,
		}
	}

	/// Get a reference to the Folder wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Folder`.
	pub fn as_folder(&self) -> Result<&FolderLayer, DocumentError> {
		match &self.data {
			LayerDataType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

	/// Get a mutable reference to the Text element wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Text`.
	pub fn as_text_mut(&mut self) -> Result<&mut TextLayer, DocumentError> {
		match &mut self.data {
			LayerDataType::Text(t) => Ok(t),
			_ => Err(DocumentError::NotText),
		}
	}

	/// Get a reference to the Text element wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Text`.
	pub fn as_text(&self) -> Result<&TextLayer, DocumentError> {
		match &self.data {
			LayerDataType::Text(t) => Ok(t),
			_ => Err(DocumentError::NotText),
		}
	}

	/// Get a mutable reference to the Image element wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Image`.
	pub fn as_image_mut(&mut self) -> Result<&mut ImageLayer, DocumentError> {
		match &mut self.data {
			LayerDataType::Image(img) => Ok(img),
			_ => Err(DocumentError::NotAnImage),
		}
	}

	/// Get a reference to the Image element wrapped by the layer.
	/// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Image`.
	pub fn as_image(&self) -> Result<&ImageLayer, DocumentError> {
		match &self.data {
			LayerDataType::Image(img) => Ok(img),
			_ => Err(DocumentError::NotAnImage),
		}
	}

	pub fn style(&self) -> Result<&PathStyle, DocumentError> {
		match &self.data {
			LayerDataType::Shape(s) => Ok(&s.style),
			LayerDataType::Text(t) => Ok(&t.path_style),
			_ => Err(DocumentError::NotAShape),
		}
	}

	pub fn style_mut(&mut self) -> Result<&mut PathStyle, DocumentError> {
		match &mut self.data {
			LayerDataType::Shape(s) => Ok(&mut s.style),
			LayerDataType::Text(t) => Ok(&mut t.path_style),
			_ => Err(DocumentError::NotAShape),
		}
	}
}

impl Clone for Layer {
	fn clone(&self) -> Self {
		Self {
			visible: self.visible,
			name: self.name.clone(),
			data: self.data.clone(),
			transform: self.transform,
			cache: String::new(),
			thumbnail_cache: String::new(),
			svg_defs_cache: String::new(),
			cache_dirty: true,
			blend_mode: self.blend_mode,
			opacity: self.opacity,
		}
	}
}

impl From<FolderLayer> for Layer {
	fn from(from: FolderLayer) -> Layer {
		Layer::new(LayerDataType::Folder(from), DAffine2::IDENTITY.to_cols_array())
	}
}

impl From<ShapeLayer> for Layer {
	fn from(from: ShapeLayer) -> Layer {
		Layer::new(LayerDataType::Shape(from), DAffine2::IDENTITY.to_cols_array())
	}
}

impl From<TextLayer> for Layer {
	fn from(from: TextLayer) -> Layer {
		Layer::new(LayerDataType::Text(from), DAffine2::IDENTITY.to_cols_array())
	}
}

impl From<ImageLayer> for Layer {
	fn from(from: ImageLayer) -> Layer {
		Layer::new(LayerDataType::Image(from), DAffine2::IDENTITY.to_cols_array())
	}
}

impl<'a> IntoIterator for &'a Layer {
	type Item = &'a Layer;
	type IntoIter = LayerIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

/// An iterator over the layers encapsulated by this layer.
/// See [Layer::iter] for more information.
#[derive(Debug, Default)]
pub struct LayerIter<'a> {
	pub stack: Vec<&'a Layer>,
}

impl<'a> Iterator for LayerIter<'a> {
	type Item = &'a Layer;

	fn next(&mut self) -> Option<Self::Item> {
		match self.stack.pop() {
			Some(layer) => {
				if let LayerDataType::Folder(folder) = &layer.data {
					let layers = folder.layers();
					self.stack.extend(layers);
				};
				Some(layer)
			}
			None => None,
		}
	}
}
