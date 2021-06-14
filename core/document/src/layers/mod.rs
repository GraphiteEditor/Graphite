pub mod style;

pub mod circle;
pub use circle::Circle;

pub mod ellipse;
pub use ellipse::Ellipse;

pub mod line;
pub use line::Line;

pub mod rect;
pub use rect::Rect;

pub mod polyline;
pub use polyline::PolyLine;

pub mod shape;
pub use shape::Shape;

pub mod folder;
pub use folder::Folder;

use std::fmt::Write;

pub trait LayerData {
	fn render(&mut self, svg: &mut String);
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
	pub visible: bool,
	pub name: Option<String>,
	pub data: LayerDataTypes,
	pub cache: String,
	pub cache_dirty: bool,
	pub blend_mode: String,
}

impl Layer {
	pub fn new(data: LayerDataTypes) -> Self {
		Self {
			visible: true,
			name: None,
			data,
			cache: String::new(),
			cache_dirty: true,
			blend_mode: "normal".to_string(),
		}
	}

	pub fn render(&mut self) -> &str {
		if !self.visible {
			return "";
		}
		if self.cache_dirty {
			self.cache.clear();
			let _ = write!(self.cache, r#"<g style = "mix-blend-mode: {}">"#, self.blend_mode);
			self.data.render(&mut self.cache);
			let _ = write!(self.cache, "</g>");
			self.cache_dirty = false;
		}
		self.cache.as_str()
	}
}
