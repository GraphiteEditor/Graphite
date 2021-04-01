mod crop;
mod ellipse;
mod line;
mod navigate;
mod path;
mod pen;
mod rectangle;
mod sample;
mod select;
mod shape;

use crate::events::{Event, ModKeys, MouseState, Trace, TracePoint};
use crate::Color;
use crate::EditorError;
use document_core::Operation;
use std::collections::HashMap;

pub trait Tool {
	fn handle_input(&mut self, event: Event) -> Vec<Operation>;
}

pub struct ToolState {
	pub mouse_state: MouseState,
	pub mod_keys: ModKeys,
	pub trace: Trace,
	pub primary_color: Color,
	pub secondary_color: Color,
	pub active_tool_type: ToolType,
	pub tools: HashMap<ToolType, Box<dyn Tool>>,
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
			active_tool_type: ToolType::Select,
			tools: gen_tools_hash_map! {
				Select => select::Select,
				Crop => crop::Crop,
				Navigate => navigate::Navigate,
				Sample => sample::Sample,
				Path => path::Path,
				Pen => pen::Pen,
				Line => line::Line,
				Rectangle => rectangle::Rectangle,
				Ellipse => ellipse::Ellipse,
				Shape => shape::Shape,
			},
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

	pub fn active_tool(&mut self) -> Result<&mut Box<dyn Tool>, EditorError> {
		self.tools.get_mut(&self.active_tool_type).ok_or(EditorError::UnknownTool)
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
