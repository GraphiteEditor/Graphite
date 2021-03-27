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
struct Trace(Vec<MouseState>);
#[derive(Debug, Clone)]
struct MouseState {
	x: u32,
	y: u32,
	mod_keys: ModKeys,
	mouse_keys: MouseKeys,
}

#[derive(Debug, Clone)]
enum Key {
	None,
}

type ModKeysStorage = u8;
type MouseKeysStorage = u8;
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct ModKeys(ModKeysStorage);

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
enum ModKey {
	Control = 1,
	Shift = 2,
	Alt = 4,
}

#[repr(u8)]
#[derive(Debug, Clone)]
enum MouseKey {
	LeftMouse = 1,
	RightMouse = 2,
	MiddleMouse = 4,
}
