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
	pub fn rotate(&self, theta: f64) -> Self {
		let cosine = theta.cos();
		let sine = theta.sin();
		Self {
			x: self.x * cosine - self.y * sine,
			y: self.x * sine + self.y * cosine,
		}
	}
	pub fn add(&self, x: f64, y: f64) -> Self {
		Self { x: self.x + x, y: self.y + y }
	}
	pub fn multiply(&self, x: f64) -> Self {
		Self { x: self.x * x, y: self.y * x }
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
	pub radians: f64,
	pub degrees: f64,
	pub scale: f64,
	pub size: ViewportPosition,
}

impl Default for CanvasTransform {
	fn default() -> Self {
		Self {
			location: CanvasPosition { x: 800., y: 800. },
			scale: 2.4,
			radians: 0.785398163, // 45 degrees in radians
			degrees: 45.,
			size: ViewportPosition::default(),
		}
	}
}

impl CanvasTransform {
	pub fn transform_string(&self) -> String {
		let inverse_scale = 1. / self.scale;
		let size = CanvasPosition {
			x: self.size.x as f64 / 2.,
			y: self.size.y as f64 / 2.,
		};
		let translation = self.location.multiply(-inverse_scale).rotate(-self.radians);
		format!(
			"translate({},{}) scale({}) rotate({})",
			translation.x + size.x,
			translation.y + size.y,
			inverse_scale,
			self.radians * -57.295779513, // 180 / pi
		)
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
	pub fn to_canvas_position(&self, canvas_transform: &CanvasTransform, apply_rotation: bool) -> CanvasPosition {
		if apply_rotation{
			CanvasPosition { x: self.x as f64, y: self.y as f64 }
				.add(canvas_transform.size.x as f64 * -0.5, canvas_transform.size.y as f64 * -0.5)
				.rotate(canvas_transform.radians)
				.multiply(canvas_transform.scale)
				.add(canvas_transform.location.x, canvas_transform.location.y)
		}else{
			let (canvas_location_x, canvas_location_y): (f64,f64) = {
				let cosine = (-canvas_transform.radians).cos();
				let sine = (-canvas_transform.radians).sin();
				(
					canvas_transform.location.x * cosine - canvas_transform.location.y * sine,
					canvas_transform.location.x * sine + canvas_transform.location.y * cosine
				)
			};
			CanvasPosition { x: self.x as f64, y: self.y as f64 }
				.add(canvas_transform.size.x as f64 * -0.5, canvas_transform.size.y as f64 * -0.5)
				//.rotate(canvas_transform.radians)
				.multiply(canvas_transform.scale)
				.add(canvas_location_x, canvas_location_y)
		}
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
