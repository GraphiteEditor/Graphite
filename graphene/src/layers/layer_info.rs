use super::blend_mode::BlendMode;
use super::folder::Folder;
use super::simple_shape::Shape;
use super::style::ViewMode;
use super::text::Text;
use crate::intersection::Quad;
use crate::DocumentError;
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Represents different types of layers.
pub enum LayerDataType {
	/// A layer that wraps a [Folder].
	Folder(Folder),
	/// A layer that wraps a [Shape].
	Shape(Shape),
	/// A layer that wraps [Text].
	Text(Text),
}

impl LayerDataType {
	pub fn inner(&self) -> &dyn LayerData {
		match self {
			LayerDataType::Shape(s) => s,
			LayerDataType::Folder(f) => f,
			LayerDataType::Text(t) => t,
		}
	}

	pub fn inner_mut(&mut self) -> &mut dyn LayerData {
		match self {
			LayerDataType::Shape(s) => s,
			LayerDataType::Folder(f) => f,
			LayerDataType::Text(t) => t,
		}
	}
}

/// Defines shared behaviour for every layer type.
pub trait LayerData {
	/// Render the layer as SVG to a given string.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::simple_shape::Shape;
	/// # use graphite_graphene::layers::style::{PathStyle, ViewMode};
	/// # use graphite_graphene::layers::layer_info::LayerData;
	/// let mut shape = Shape::rectangle(PathStyle::new(None, None));
	/// let mut svg = String::new();
    ///
	/// // Render the shape without any transforms, in normal view mode
	/// shape.render(&mut svg, &mut vec![], ViewMode::Normal);
    ///
	/// assert_eq!(
    ///     svg, 
    ///     "<g transform=\"matrix(\n1,-0,-0,1,-0,-0)\">\
    ///     <path d=\"M0 0L1 0L1 1L0 1Z\"  fill=\"none\" />\
    ///     </g>"
    /// );
	/// ```
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<glam::DAffine2>, view_mode: ViewMode);

    // FIXME: finish this test
    /// Determine the layers within this layer that intersect a given quad.
	/// # Example
	/// ```no_run
	/// # use graphite_graphene::layers::simple_shape::Shape;
	/// # use graphite_graphene::layers::style::{PathStyle, ViewMode};
	/// # use graphite_graphene::layers::layer_info::LayerData;
	/// # use graphite_graphene::intersection::Quad;
    /// # use glam::f64::{DAffine2, DVec2};
	/// let mut shape = Shape::ellipse(PathStyle::new(None, None));
	/// let mut svg = String::new();
    ///
    /// let quad = Quad::from_box([DVec2::ZERO, DVec2::ONE]);
    /// let mut intersections = vec![];
    ///
    /// // Since we are not working with a Folder layer, the path is empty
    /// let mut path = vec![];
    ///
	/// shape.intersects_quad(quad, &mut path, &mut intersections);
    ///
	/// assert_eq!(intersections, vec![vec![0_u64]]);
	/// ```
	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>);

    // FIXME: this doctest fails because 0 != 1E-32
	/// Calculate the bounding box for the layers contents after applying a given transform.
	/// # Example
	/// ```no_run
	/// # use graphite_graphene::layers::simple_shape::Shape;
	/// # use graphite_graphene::layers::style::PathStyle;
	/// # use graphite_graphene::layers::layer_info::LayerData;
    /// # use glam::f64::{DAffine2, DVec2};
	/// let shape = Shape::ellipse(PathStyle::new(None, None));
    ///
    /// // Calculate the bounding box without applying any transformations.
    /// // (The identity transform maps every vector to itself)
    /// let transform = DAffine2::IDENTITY;
    /// let bounding_box = shape.bounding_box(transform);
    ///
	/// assert_eq!(bounding_box, Some([DVec2::ZERO, DVec2::ONE]));
	/// ```
	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]>;
}

impl LayerData for LayerDataType {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<glam::DAffine2>, view_mode: ViewMode) {
		self.inner_mut().render(svg, transforms, view_mode)
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		self.inner().intersects_quad(quad, path, intersections)
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		self.inner().bounding_box(transform)
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
	// No, there is no smarter way to do this.
	true
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Layer {
	/// Whether or not the layer is currently visible.
	pub visible: bool,
	/// The name of the layer.
	pub name: Option<String>,
	/// The type of layer.
	pub data: LayerDataType,
	/// A transformation applied to the layer(translation, rotation, scaling and shear).
	#[serde(with = "DAffine2Ref")]
	pub transform: glam::DAffine2,
	#[serde(skip)]
	pub cache: String,
	#[serde(skip)]
	pub thumbnail_cache: String,
	/// Whether or not the [Cache](Layer::cache) and [Thumbnail Cache](Layer::thumbnail_cache) need to be updated.
	#[serde(skip, default = "return_true")]
	pub cache_dirty: bool,
	/// Describes how overlapping SVG elements should be blended together.
	pub blend_mode: BlendMode,
	/// The opacity of the Layer, always ∈ [0, 1].
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
			cache_dirty: true,
			blend_mode: BlendMode::Normal,
			opacity: 1.,
		}
	}

    /// Iterate over the layers encapsulated by this layer.
    /// If the [Layer Type](Layer::data) is [Text](LayerDataType::Text) or [Shape](LayerDataType::Shape),
    /// the only item in the iterator will be the layer itself.
    /// If the [Layer Type](Layer::data) wraps a [Folder](LayerDataType::Folder), the iterator
    /// will recursively yield all the layers contained in the folder as well as potential sub-folders.
	pub fn iter(&self) -> LayerIter<'_> {
		LayerIter { stack: vec![self] }
	}

	pub fn render(&mut self, transforms: &mut Vec<DAffine2>, view_mode: ViewMode) -> &str {
		if !self.visible {
			return "";
		}

		if self.cache_dirty {
			transforms.push(self.transform);
			self.thumbnail_cache.clear();
			self.data.render(&mut self.thumbnail_cache, transforms, view_mode);

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
			transforms.pop();
			self.cache_dirty = false;
		}

		self.cache.as_str()
	}

	pub fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if !self.visible {
			return;
		}

		let transformed_quad = self.transform.inverse() * quad;
		self.data.intersects_quad(transformed_quad, path, intersections)
	}

	pub fn current_bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.data.bounding_box(transform)
	}

	pub fn current_bounding_box(&self) -> Option<[DVec2; 2]> {
		self.current_bounding_box_with_transform(self.transform)
	}

    /// Get a mutable reference to the Folder wrapped by the layer.
    /// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Folder`.
	pub fn as_folder_mut(&mut self) -> Result<&mut Folder, DocumentError> {
		match &mut self.data {
			LayerDataType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

    /// Get a reference to the Folder wrapped by the layer.
    /// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Folder`.
	pub fn as_folder(&self) -> Result<&Folder, DocumentError> {
		match &self.data {
			LayerDataType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

    /// Get a mutable reference to the Text element wrapped by the layer.
    /// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Text`.
	pub fn as_text_mut(&mut self) -> Result<&mut Text, DocumentError> {
		match &mut self.data {
			LayerDataType::Text(t) => Ok(t),
			_ => Err(DocumentError::NotText),
		}
	}

    /// Get a reference to the Text element wrapped by the layer.
    /// This operation will fail if the [Layer type](Layer::data) is not `LayerDataType::Text`.
	pub fn as_text(&self) -> Result<&Text, DocumentError> {
		match &self.data {
			LayerDataType::Text(t) => Ok(t),
			_ => Err(DocumentError::NotText),
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
			cache_dirty: true,
			blend_mode: self.blend_mode,
			opacity: self.opacity,
		}
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
