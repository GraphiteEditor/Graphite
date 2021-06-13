pub mod style;

pub mod circle;
pub use circle::Circle;

pub mod ellipse;
pub use ellipse::Ellipse;

pub mod line;
use kurbo::Point;
pub use line::Line;

pub mod rect;
pub use rect::Rect;

pub mod polyline;
pub use polyline::PolyLine;

pub mod shape;
pub use shape::Shape;

pub mod folder;
pub use folder::Folder;

use crate::LayerId;

pub const KURBO_TOLERANCE: f64 = 0.0001;

pub trait LayerData {
	fn render(&mut self, svg: &mut String);
	fn intersects_quad(&self, quad: [Point; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>);
	fn intersects_point(&self, point: Point, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>);
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerDataTypes {
	Folder(Folder),
	Circle(Circle),
	Ellipse(Ellipse),
	Rect(Rect),
	Line(Line),
	PolyLine(PolyLine),
	Shape(Shape),
}

macro_rules! call_render {
    ($self:ident.render($svg:ident) { $($variant:ident),* }) => {
		match $self {
			$(Self::$variant(x) => x.render($svg)),*
		}
	};
}

macro_rules! call_intersects_quad {
    ($self:ident.intersects_quad($quad:ident, $path:ident, $intersections:ident) { $($variant:ident),* }) => {
		match $self {
			$(Self::$variant(x) => x.intersects_quad($quad, $path, $intersections)),*
		}
	};
}

macro_rules! call_intersects_point {
    ($self:ident.intersects_point($point:ident, $path:ident, $intersections:ident) { $($variant:ident),* }) => {
		match $self {
			$(Self::$variant(x) => x.intersects_point($point, $path, $intersections)),*
		}
	};
}

impl LayerDataTypes {
	pub fn render(&mut self, svg: &mut String) {
		call_render! {
			self.render(svg) {
				Folder,
				Circle,
				Ellipse,
				Rect,
				Line,
				PolyLine,
				Shape
			}
		}
	}

	pub fn intersects_quad(&self, quad: [Point; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		call_intersects_quad! {
			self.intersects_quad(quad, path, intersections) {
				Folder,
				Circle,
				Ellipse,
				Rect,
				Line,
				PolyLine,
				Shape
			}
		}
	}

	pub fn intersects_point(&self, point: Point, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		call_intersects_point! {
			self.intersects_point(point, path, intersections) {
				Folder,
				Circle,
				Ellipse,
				Rect,
				Line,
				PolyLine,
				Shape
			}
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
	pub visible: bool,
	pub name: Option<String>,
	pub data: LayerDataTypes,
	pub cache: String,
	pub cache_dirty: bool,
}

impl Layer {
	pub fn new(data: LayerDataTypes) -> Self {
		Self {
			visible: true,
			name: None,
			data,
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
			self.data.render(&mut self.cache);
			self.cache_dirty = false;
		}
		self.cache.as_str()
	}

	pub fn intersects_quad(&self, quad: [Point; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		// TODO: apply transform to quad
		if !self.visible {
			return;
		}
		self.data.intersects_quad(quad, path, intersections)
	}

	pub fn intersects_point(&self, point: Point, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		// TODO: apply transform to point
		if !self.visible {
			return;
		}
		self.data.intersects_point(point, path, intersections)
	}
}
