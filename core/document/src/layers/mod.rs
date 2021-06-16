pub mod style;

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
	pub transform: glam::DAffine2,
	pub cache: String,
	pub cache_dirty: bool,
}

impl Layer {
	pub fn new(data: LayerDataTypes, transform: glam::DAffine2) -> Self {
		Self {
			visible: true,
			name: None,
			data,
			transform,
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
			let _ = write!(self.cache, r#"<g transform="matrix("#);
			self.transform.to_cols_array().iter().enumerate().for_each(|(i, f)| {
				let _ = write!(self.cache, "{}{}", f, if i != 5 { "," } else { "" });
			});
			let _ = writeln!(self.cache, r#")">"#);

			self.data.render(&mut self.cache);
			let _ = write!(self.cache, "</g>");
			self.cache_dirty = false;
		}
		self.cache.as_str()
	}
}
