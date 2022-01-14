use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum ToolOptions {
	Select { append_mode: SelectAppendMode },
	Crop {},
	Navigate {},
	Eyedropper {},
	Text {},
	Fill {},
	Gradient {},
	Brush {},
	Heal {},
	Clone {},
	Patch {},
	BlurSharpen {},
	Relight {},
	Path {},
	Pen { weight: u32 },
	Freehand {},
	Spline {},
	Line { weight: u32 },
	Rectangle {},
	Ellipse {},
	Shape { shape_type: ShapeType },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum SelectAppendMode {
	New,
	Add,
	Subtract,
	Intersect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum ShapeType {
	Star { vertices: u32 },
	Polygon { vertices: u32 },
}
