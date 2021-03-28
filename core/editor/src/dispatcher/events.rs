use crate::tools::ToolType;
#[derive(Debug, Clone)]
#[repr(C)]
pub enum Event {
	SelectTool(ToolType),
	ModifierKeyDown(ModKey),
	ModifierKeyUp(ModKey),
	MouseMovement(Trace),
	Click(MouseState),
	KeyPress(Key),
}

#[derive(Debug, Clone)]
#[repr(C)]
pub enum Response {
	UpdateCanvas,
}

#[derive(Debug, Clone)]
pub struct Trace(Vec<MouseState>);

impl Trace {
	pub fn new() -> Self {
		Self(vec![])
	}
	pub fn first_point(&self) -> Option<&MouseState> {
		if !self.0.is_empty() {
			Some(&self.0[0])
		} else {
			None
		}
	}
	pub fn last_point(&self) -> Option<&MouseState> {
		let trace = &self.0;
		let trace_length = trace.len();
		if trace_length > 0 {
			Some(&trace[trace_length - 1])
		} else {
			None
		}
	}
	pub fn append_point(&mut self, x: u32, y: u32) {
		self.0.push(MouseState::from_pos(x, y))
	}
	pub fn clear(&mut self) {
		self.0.clear()
	}
}
#[derive(Debug, Clone, Default)]
pub struct MouseState {
	x: u32,
	y: u32,
	mod_keys: ModKeysStorage,
	mouse_keys: MouseKeysStorage,
}

impl MouseState {
	pub const fn new() -> MouseState {
		MouseState {
			x: 0,
			y: 0,
			mod_keys: 0,
			mouse_keys: 0,
		}
	}
	pub const fn from_pos(x: u32, y: u32) -> MouseState {
		MouseState { x, y, mod_keys: 0, mouse_keys: 0 }
	}
}

#[derive(Debug, Clone)]
pub enum Key {
	None,
}

pub type ModKeysStorage = u8;
pub type MouseKeysStorage = u8;
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ModKeys(ModKeysStorage);

impl ModKeys {
	pub fn get_key(&self, key: ModKey) -> bool {
		key as ModKeysStorage & self.0 > 0
	}
	pub fn set_key(&mut self, key: ModKey) {
		self.0 |= key as ModKeysStorage
	}
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct MouseKeys(u8);

impl MouseKeys {
	pub fn get_key(&self, key: MouseKey) -> bool {
		key as ModKeysStorage & self.0 > 0
	}
	pub fn set_key(&mut self, key: MouseKey) {
		self.0 |= key as MouseKeysStorage
	}
}

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum ModKey {
	Control = 1,
	Shift = 2,
	Alt = 4,
}

#[repr(u8)]
#[derive(Debug, Clone)]
pub enum MouseKey {
	LeftMouse = 1,
	RightMouse = 2,
	MiddleMouse = 4,
}
