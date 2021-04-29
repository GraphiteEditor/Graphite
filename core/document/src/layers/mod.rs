pub mod style;

pub mod circle;
pub use circle::Circle;

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
	fn render(&self) -> String;
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerDataTypes {
	Folder(Folder),
	Circle(Circle),
	Rect(Rect),
	Line(Line),
	PolyLine(PolyLine),
	Shape(Shape),
}

impl LayerDataTypes {
	pub fn render(&self) -> String {
		match self {
			Self::Folder(f) => f.render(),
			Self::Circle(c) => c.render(),
			Self::Rect(r) => r.render(),
			Self::Line(l) => l.render(),
			Self::PolyLine(pl) => pl.render(),
			Self::Shape(s) => s.render(),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
	pub visible: bool,
	pub name: Option<String>,
	pub data: LayerDataTypes,
}

impl Layer {
	pub fn new(data: LayerDataTypes) -> Self {
		Self { visible: true, name: None, data }
	}
}
