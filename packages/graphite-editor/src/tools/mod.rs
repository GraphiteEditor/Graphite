use crate::Color;

const TOOL_COUNT: usize = 10;

struct ToolState {
	primary_color: Color,
	secondary_color: Color,
	active_tool: ToolType,
	tool_settings: [ToolSettings; TOOL_COUNT],
}

impl ToolState {
	pub fn select_tool(&mut self, tool: ToolType) {
		self.active_tool = ToolType
	}
}

#[repr(usize)]
enum ToolType {
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

enum ToolSettings {
	Select { append_mode: SelectAppendMode },
}

enum SelectAppendMode {
	New,
	Add,
	Substract,
	Intersect,
}
