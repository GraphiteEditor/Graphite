use crate::events::{ModKeys, MouseState, TracePoint};
use crate::{events::Trace, Color};
use std::collections::HashMap;

pub struct ToolState {
	pub mouse_state: MouseState,
	pub mod_keys: ModKeys,
	pub trace: Trace,
	pub primary_color: Color,
	pub secondary_color: Color,
	pub active_tool: ToolType,
	tool_settings: HashMap<ToolType, ToolSettings>,
}

impl Default for ToolState {
	fn default() -> Self {
		ToolState {
			mouse_state: MouseState::default(),
			mod_keys: ModKeys::default(),
			trace: Trace::new(),
			primary_color: Color::BLACK,
			secondary_color: Color::WHITE,
			active_tool: ToolType::Select,
			tool_settings: default_tool_settings(),
		}
	}
}

impl ToolState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn record_trace_point(&mut self) {
		self.trace.push(TracePoint {
			mouse_state: self.mouse_state,
			mod_keys: self.mod_keys,
		})
	}
}

fn default_tool_settings() -> HashMap<ToolType, ToolSettings> {
	let tool_init = |tool: &ToolType| (*tool, tool.default_settings());
	// TODO: when 1.51 is more common, change this to use array::IntoIter
	[
		tool_init(&ToolType::Select),
		tool_init(&ToolType::Ellipse),
		tool_init(&ToolType::Shape), // TODO: Add more tool defaults
	]
	.iter()
	.copied()
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
			ToolType::Ellipse => ToolSettings::Ellipse,
			ToolType::Shape => ToolSettings::Shape {
				shape: Shape::Polygon { vertices: 3 },
			},
			_ => todo!(),
		}
	}
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ToolSettings {
	Select { append_mode: SelectAppendMode },
	Ellipse,
	Shape { shape: Shape },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SelectAppendMode {
	New,
	Add,
	Subtract,
	Intersect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Shape {
	Star { vertices: u32 },
	Polygon { vertices: u32 },
}
