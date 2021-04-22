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

use crate::events::{Event, ModKeys, MouseState, Response, Trace, TracePoint};
use crate::Color;
use crate::Document;
use crate::EditorError;
use document_core::Operation;
use std::{collections::HashMap, fmt};

pub trait Tool {
	fn handle_input(&mut self, event: &Event, document: &Document, tool_data: &DocumentToolData) -> (Vec<Response>, Vec<Operation>);
}

pub trait Fsm {
	type ToolData;
	fn transition(self, event: &Event, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> Self;
}

pub struct DocumentToolData {
	pub mouse_state: MouseState,
	pub mod_keys: ModKeys,
	pub primary_color: Color,
	pub secondary_color: Color,
}
pub struct ToolData {
	pub active_tool_type: ToolType,
	pub tools: HashMap<ToolType, Box<dyn Tool>>,
	tool_settings: HashMap<ToolType, ToolSettings>,
}

impl ToolData {
	pub fn active_tool(&mut self) -> Result<&mut Box<dyn Tool>, EditorError> {
		self.tools.get_mut(&self.active_tool_type).ok_or(EditorError::UnknownTool)
	}
}

pub struct ToolFsmState {
	pub document_tool_data: DocumentToolData,
	pub tool_data: ToolData,
	pub trace: Trace,
}

impl Default for ToolFsmState {
	fn default() -> Self {
		ToolFsmState {
			trace: Trace::new(),
			tool_data: ToolData {
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
			},
			document_tool_data: DocumentToolData {
				mouse_state: MouseState::default(),
				mod_keys: ModKeys::default(),
				primary_color: Color::BLACK,
				secondary_color: Color::WHITE,
			},
		}
	}
}

impl ToolFsmState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn record_trace_point(&mut self) {
		self.trace.push(TracePoint {
			mouse_state: self.document_tool_data.mouse_state,
			mod_keys: self.document_tool_data.mod_keys,
		})
	}

	pub fn swap_colors(&mut self) {
		std::mem::swap(&mut self.document_tool_data.primary_color, &mut self.document_tool_data.secondary_color);
	}
}

fn default_tool_settings() -> HashMap<ToolType, ToolSettings> {
	let tool_init = |tool: &ToolType| (*tool, tool.default_settings());
	std::array::IntoIter::new([
		tool_init(&ToolType::Select),
		tool_init(&ToolType::Ellipse),
		tool_init(&ToolType::Shape), // TODO: Add more tool defaults
	])
	.collect()
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
	Select,
	Crop,
	Navigate,
	Sample,
	Text,
	Fill,
	Gradient,
	Brush,
	Heal,
	Clone,
	Patch,
	BlurSharpen,
	Relight,
	Path,
	Pen,
	Freehand,
	Spline,
	Line,
	Rectangle,
	Ellipse,
	Shape,
}

impl fmt::Display for ToolType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		use ToolType::*;

		let name = match_variant_name!(match (self) {
			Select,
			Crop,
			Navigate,
			Sample,
			Text,
			Fill,
			Gradient,
			Brush,
			Heal,
			Clone,
			Patch,
			BlurSharpen,
			Relight,
			Path,
			Pen,
			Freehand,
			Spline,
			Line,
			Rectangle,
			Ellipse,
			Shape
		});

		formatter.write_str(name)
	}
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
