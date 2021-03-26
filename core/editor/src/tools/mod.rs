use crate::Color;

const TOOL_COUNT: usize = 10;

pub struct ToolState {
	primary_color: Color,
	secondary_color: Color,
	active_tool: ToolType,
	tool_settings: [ToolSettings; TOOL_COUNT],
}

impl ToolState {
	pub const fn default() -> ToolState {
		ToolState {
			primary_color: Color::BLACK,
			secondary_color: Color::WHITE,
			active_tool: ToolType::Select,
			tool_settings: [ToolSettings::Select { append_mode: SelectAppendMode::New }; TOOL_COUNT],
			// TODO: Initialize to sensible values
		}
	}
	pub fn select_tool(&mut self, tool: ToolType) {
		self.active_tool = tool
	}
	pub fn set_primary_color<T: Into<Color>>(&mut self, primary_color: T) {
		self.primary_color = primary_color.into();
	}
	pub fn set_secondary_color<T: Into<Color>>(&mut self, secondary_color: T) {
		self.secondary_color = secondary_color.into();
	}
}

#[repr(usize)]
#[derive(Debug, Clone)]
pub enum ToolType {
	Select = 0,
	Crop = 1,
	Navigate = 2,
	Sample = 3,
	Path = 4,
	Pen = 5,
	Line = 6,
	Rectangle = 7,
	Ellipse = 8,
	Shape = 9,
	// all discriminats must be strictly smaller than TOOL_COUNT!
}

#[derive(Debug, Clone, Copy)]
pub enum ToolSettings {
	Select { append_mode: SelectAppendMode },
}

#[derive(Debug, Clone, Copy)]
pub enum SelectAppendMode {
	New,
	Add,
	Subtract,
	Intersect,
}
