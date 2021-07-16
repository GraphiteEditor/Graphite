pub mod style;

pub mod ellipse;
pub use ellipse::Ellipse;

pub mod line;
use glam::{DMat2, DVec2};
use kurbo::BezPath;
pub use line::Line;

pub mod rect;
pub use rect::Rect;

pub mod polyline;
pub use polyline::PolyLine;

pub mod shape;
pub use shape::Shape;

pub mod folder;
use crate::DocumentError;
use crate::LayerId;
pub use folder::Folder;
use serde::{Deserialize, Serialize};

pub trait LayerData {
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle);
	fn to_kurbo_path(&self, transform: glam::DAffine2, style: style::PathStyle) -> BezPath;
	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle);
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum LayerDataTypes {
	Folder(Folder),
	Ellipse(Ellipse),
	Rect(Rect),
	Line(Line),
	PolyLine(PolyLine),
	Shape(Shape),
}

macro_rules! call_render {
	($self:ident.render($svg:ident, $transform:ident, $style:ident) { $($variant:ident),* }) => {
		match $self {
			$(Self::$variant(x) => x.render($svg, $transform, $style)),*
		}
	};
}
macro_rules! call_kurbo_path {
	($self:ident.to_kurbo_path($transform:ident, $style:ident) { $($variant:ident),* }) => {
		match $self {
			$(Self::$variant(x) => x.to_kurbo_path($transform, $style)),*
		}
	};
}

macro_rules! call_intersects_quad {
    ($self:ident.intersects_quad($quad:ident, $path:ident, $intersections:ident, $style:ident) { $($variant:ident),* }) => {
		match $self {
			$(Self::$variant(x) => x.intersects_quad($quad, $path, $intersections, $style)),*
		}
	};
}

impl LayerDataTypes {
	pub fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		call_render! {
			self.render(svg, transform, style) {
				Folder,
				Ellipse,
				Rect,
				Line,
				PolyLine,
				Shape
			}
		}
	}
	pub fn to_kurbo_path(&self, transform: glam::DAffine2, style: style::PathStyle) -> BezPath {
		call_kurbo_path! {
			self.to_kurbo_path(transform, style) {
				Folder,
				Ellipse,
				Rect,
				Line,
				PolyLine,
				Shape
			}
		}
	}

	pub fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		call_intersects_quad! {
			self.intersects_quad(quad, path, intersections, style) {
				Folder,
				Ellipse,
				Rect,
				Line,
				PolyLine,
				Shape
			}
		}
	}

	pub fn bounding_box(&self, transform: glam::DAffine2, style: style::PathStyle) -> [DVec2; 2] {
		use kurbo::Shape;
		let bez_path = self.to_kurbo_path(transform, style);
		let bbox = bez_path.bounding_box();
		[DVec2::new(bbox.x0, bbox.y0), DVec2::new(bbox.x1, bbox.y1)]
	}
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "glam::DAffine2")]
struct DAffine2Ref {
	pub matrix2: DMat2,
	pub translation: DVec2,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Layer {
	pub visible: bool,
	pub name: Option<String>,
	pub data: LayerDataTypes,
	#[serde(with = "DAffine2Ref")]
	pub transform: glam::DAffine2,
	pub style: style::PathStyle,
	pub cache: String,
	pub cache_dirty: bool,
}

impl Layer {
	pub fn new(data: LayerDataTypes, transform: [f64; 6], style: style::PathStyle) -> Self {
		Self {
			visible: true,
			name: None,
			data,
			transform: glam::DAffine2::from_cols_array(&transform),
			style: style,
			cache: String::new(),
			cache_dirty: true,
		}
	}

	pub fn render(&mut self) -> &str {
		if !self.visible {
			return "";
		}
		if self.cache_dirty {
			self.cache.clear();
			self.data.render(&mut self.cache, self.transform, self.style);
			self.cache_dirty = false;
		}
		self.cache.as_str()
	}

	pub fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		let inv_transform = self.transform.inverse();
		let transformed_quad = [
			inv_transform.transform_point2(quad[0]),
			inv_transform.transform_point2(quad[1]),
			inv_transform.transform_point2(quad[2]),
			inv_transform.transform_point2(quad[3]),
		];
		if !self.visible {
			return;
		}
		self.data.intersects_quad(transformed_quad, path, intersections, self.style)
	}

	pub fn render_on(&mut self, svg: &mut String) {
		*svg += self.render();
	}

	pub fn to_kurbo_path(&mut self) -> BezPath {
		self.data.to_kurbo_path(self.transform, self.style)
	}

	pub fn bounding_box(&self, transform: glam::DAffine2, style: style::PathStyle) -> [DVec2; 2] {
		self.data.bounding_box(transform, style)
	}

	pub fn as_folder_mut(&mut self) -> Result<&mut Folder, DocumentError> {
		match &mut self.data {
			LayerDataTypes::Folder(f) => Ok(f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

	pub fn as_folder(&self) -> Result<&Folder, DocumentError> {
		match &self.data {
			LayerDataTypes::Folder(f) => Ok(&f),
			_ => Err(DocumentError::NotAFolder),
		}
	}

	pub fn render_as_folder(&mut self, svg: &mut String) {
		match &mut self.data {
			LayerDataTypes::Folder(f) => f.render(svg, self.transform, self.style),
			_ => {}
		}
	}
}
