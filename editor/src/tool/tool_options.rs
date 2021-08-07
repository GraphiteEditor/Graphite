use serde::{Deserialize, Serialize};

// TODO: Rename this `ToolOption` to not be plural in a separate commit (together with `enum LayerDataTypes`)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum ToolOptions {
	Select { append_mode: SelectAppendMode },
	Ellipse,
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
