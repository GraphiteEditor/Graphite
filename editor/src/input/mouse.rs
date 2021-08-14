use bitflags::bitflags;
use glam::DVec2;

// Origin is top left
pub type ViewportPosition = DVec2;

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash)]
pub struct ScrollDelta {
	pub x: i32,
	pub y: i32,
	pub z: i32,
}
impl ScrollDelta {
	pub fn new(x: i32, y: i32, z: i32) -> ScrollDelta {
		ScrollDelta { x, y, z }
	}
	pub fn as_dvec2(&self) -> DVec2 {
		DVec2::new(self.x as f64, self.y as f64)
	}
	pub fn scroll_delta(&self) -> f64 {
		let (dx, dy) = (self.x, self.y);
		dy.signum() as f64 * ((dy * dy + i32::min(dy.abs(), dx.abs()).pow(2)) as f64).sqrt()
	}
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct MouseState {
	pub position: ViewportPosition,
	pub mouse_keys: MouseKeys,
	pub scroll_delta: ScrollDelta,
}

impl MouseState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_pos(x: f64, y: f64) -> Self {
		Self {
			position: (x, y).into(),
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
		const NONE   = 0b0000_0000;
	}
}
