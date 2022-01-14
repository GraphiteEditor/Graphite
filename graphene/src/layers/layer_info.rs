use super::blend_mode::BlendMode;
use super::folder::Folder;
use super::simple_shape::Shape;
use super::style::ViewMode;
use crate::intersection::Quad;
use crate::DocumentError;
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum LayerDataType {
	Folder(Folder),
	Shape(Shape),
}

impl LayerDataType {
	pub fn inner(&self) -> &dyn LayerData {
		match self {
			LayerDataType::Shape(s) => s,
			LayerDataType::Folder(f) => f,
		}
	}

	pub fn inner_mut(&mut self) -> &mut dyn LayerData {
		match self {
			LayerDataType::Shape(s) => s,
			LayerDataType::Folder(f) => f,
		}
	}
}

pub trait LayerData {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<glam::DAffine2>, view_mode: ViewMode);
	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>);
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

fn return_true() -> bool {
	true
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Layer {
	pub visible: bool,
	pub name: Option<String>,
	pub data: LayerDataType,
	#[serde(with = "DAffine2Ref")]
	pub transform: glam::DAffine2,
	#[serde(skip)]
	pub cache: String,
	#[serde(skip)]
	pub thumbnail_cache: String,
	#[serde(skip, default = "return_true")]
	pub cache_dirty: bool,
	pub blend_mode: BlendMode,
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
				let _ = self.cache.write_str(&(f.to_string() + if i != 5 { "," } else { "" }));
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

	pub fn as_folder_mut(&mut self) -> Result<&mut Folder, DocumentError> {
		match &mut self.data {
			LayerDataType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

	pub fn as_folder(&self) -> Result<&Folder, DocumentError> {
		match &self.data {
			LayerDataType::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
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
