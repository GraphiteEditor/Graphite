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
		log::debug!("SVG: {:?}", svg);
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
}
