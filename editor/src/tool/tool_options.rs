use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum ToolOptions {
	Select { append_mode: SelectAppendMode },
	Ellipse,
	Shape { shape_type: ShapeType },
	Line { weight: u32 },
	Pen { weight: u32 },
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
