use bitflags::bitflags;
use glam::DVec2;

// origin is top left
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct ViewportPosition {
	pub x: u32,
	pub y: u32,
}

impl ViewportPosition {
	pub fn distance(&self, other: &Self) -> f64 {
		let x_diff = other.x as i64 - self.x as i64;
		let y_diff = other.y as i64 - self.y as i64;
		f64::sqrt((x_diff * x_diff + y_diff * y_diff) as f64)
	}
	pub fn to_dvec2(&self) -> DVec2 {
		DVec2::new(self.x as f64, self.y as f64)
	}
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct ScrollDelta {
	pub x: i32,
	pub y: i32,
	pub z: i32,
}
impl ScrollDelta {
	pub fn new(x: i32, y: i32, z: i32) -> ScrollDelta {
		ScrollDelta { x, y, z }
	}
	pub fn to_dvec2(&self) -> DVec2 {
		DVec2::new(self.x as f64, self.y as f64)
	}
	pub fn scroll_delta(&self) -> f64 {
		let (dx, dy) = (self.x, self.y);
		dy.signum() as f64 * ((dy * dy + i32::min(dy.abs(), dx.abs()).pow(2)) as f64).sqrt()
	}
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct MouseState {
	pub position: ViewportPosition,
	pub mouse_keys: MouseKeys,
	pub scroll_delta: ScrollDelta,
}

impl MouseState {
	pub fn new() -> MouseState {
		Self::default()
	}

	pub fn from_pos(x: u32, y: u32) -> MouseState {
		MouseState {
			position: ViewportPosition { x, y },
			mouse_keys: MouseKeys::default(),
			scroll_delta: ScrollDelta::default(),
		}
	}
	pub fn from_u8_pos(keys: u8, position: ViewportPosition) -> Self {
		let mouse_keys = MouseKeys::from_bits(keys).expect("invalid modifier keys");
		Self {
			position,
			mouse_keys,
			scroll_delta: ScrollDelta::default(),
		}
	}
}
bitflags! {
	#[derive(Default)]
	#[repr(transparent)]
	pub struct MouseKeys: u8 {
		const LEFT   = 0b0000_0001;
		const RIGHT  = 0b0000_0010;
		const MIDDLE = 0b0000_0100;
	}
}
