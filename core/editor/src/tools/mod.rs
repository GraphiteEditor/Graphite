use crate::{events::Trace, Color};
use std::collections::HashMap;

pub struct ToolState {
	pub mouse_is_clicked: bool,
	pub trace: Trace,
	pub primary_color: Color,
	pub secondary_color: Color,
	pub active_tool: ToolType,
	tool_settings: HashMap<ToolType, ToolSettings>,
}

impl ToolState {
	pub fn new() -> Self {
		ToolState {
			mouse_is_clicked: false,
			trace: Trace::new(),
			primary_color: Color::BLACK,
			secondary_color: Color::WHITE,
			active_tool: ToolType::Select,
			tool_settings: default_tool_settings(),
		}
	}
}

fn default_tool_settings() -> HashMap<ToolType, ToolSettings> {
	let tool_init = |tool: &ToolType| (*tool, tool.default_settings());
	[
		tool_init(&ToolType::Select),
		tool_init(&ToolType::Shape), // TODO: Add more tool defaults
	]
	.iter()
	.cloned()
	.collect()
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
	Select,
	Crop,
	Navigate,
	Sample,
	Path,
	Pen,
	Line,
	Rectangle,
	Ellipse,
	Shape,
	// all discriminats must be strictly smaller than TOOL_COUNT!
}

impl ToolType {
	fn default_settings(&self) -> ToolSettings {
		match self {
			ToolType::Select => ToolSettings::Select { append_mode: SelectAppendMode::New },
			ToolType::Shape => ToolSettings::Shape {
				shape: Shape::Polygon { vertices: 3 },
			},
			_ => todo!(),
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub enum ToolSettings {
	Select { append_mode: SelectAppendMode },
	Shape { shape: Shape },
}

#[derive(Debug, Clone, Copy)]
pub enum SelectAppendMode {
	New,
	Add,
	Subtract,
	Intersect,
}

#[derive(Debug, Clone, Copy)]
pub enum Shape {
	Star { vertices: u32 },
	Polygon { vertices: u32 },
}
