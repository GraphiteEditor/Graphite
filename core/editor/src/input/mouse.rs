use bitflags::bitflags;

// A position on the infinite canvas
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct CanvasPosition {
	pub x: f64,
	pub y: f64,
}

impl CanvasPosition {
	pub fn distance(&self, other: &Self) -> f64 {
		let x_diff = other.x - self.x;
		let y_diff = other.y - self.y;
		f64::sqrt(x_diff * x_diff + y_diff * y_diff)
	}
	pub fn rotate(&mut self, theta: f64) -> &mut Self {
		let cosine = theta.cos();
		let sine = theta.sin();
		self.x = self.x * cosine - self.y * sine;
		self.y = self.x * sine + self.y * cosine;
		self
	}
}
impl From<CanvasPosition> for (f64, f64) {
	fn from(item: CanvasPosition) -> Self {
		(item.x, item.y)
	}
}

// The location of the viewport (or anything else) in the canvas
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CanvasTransform {
	pub location: CanvasPosition,
	pub rotation: f64,
	pub scale: f64,
	pub center: ViewportPosition,
}

impl Default for CanvasTransform {
	fn default() -> Self {
		Self {
			location: CanvasPosition { x: 100., y: 100. },
			scale: 1.,
			rotation: 0.,
			center: ViewportPosition::default(),
		}
	}
}

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
	pub fn to_canvas_position(&self, canvas_transform: &CanvasTransform) -> CanvasPosition {
		*CanvasPosition {
			x: (self.x - canvas_transform.center.x) as f64 * canvas_transform.scale + canvas_transform.location.x,
			y: (self.y - canvas_transform.center.y) as f64 * canvas_transform.scale + canvas_transform.location.y,
		}
		.rotate(canvas_transform.rotation)
	}
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct MouseState {
	pub position: ViewportPosition,
	pub mouse_keys: MouseKeys,
}

impl MouseState {
	pub fn new() -> MouseState {
		Self::default()
	}

	pub fn from_pos(x: u32, y: u32) -> MouseState {
		MouseState {
			position: ViewportPosition { x, y },
			mouse_keys: MouseKeys::default(),
		}
	}
	pub fn from_u8_pos(keys: u8, position: ViewportPosition) -> Self {
		let mouse_keys = MouseKeys::from_bits(keys).expect("invalid modifier keys");
		Self { position, mouse_keys }
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
