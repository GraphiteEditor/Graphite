use std::ops::{Add, Mul};

use bitflags::bitflags;

// A position on the infinite canvas
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct DocumentPosition {
	pub x: f64,
	pub y: f64,
}

impl DocumentPosition {
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
}
impl Mul<f64> for DocumentPosition {
	// The multiplication of rational numbers is a closed operation.
	type Output = Self;

	fn mul(self, rhs: f64) -> Self {
		Self { x: self.x * rhs, y: self.y * rhs }
	}
}
impl Add<(f64, f64)> for DocumentPosition {
	type Output = DocumentPosition;
	fn add(self, rhs: (f64, f64)) -> Self {
		Self { x: self.x + rhs.0, y: self.y + rhs.1 }
	}
}
impl From<DocumentPosition> for (f64, f64) {
	fn from(item: DocumentPosition) -> Self {
		(item.x, item.y)
	}
}

// The location of the viewport (or anything else) in the canvas
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DocumentTransform {
	pub location: DocumentPosition,
	pub radians: f64,
	pub degrees: f64,
	pub scale: f64,
	pub size: ViewportPosition,
}

impl Default for DocumentTransform {
	fn default() -> Self {
		Self {
			location: DocumentPosition { x: 800., y: 800. },
			scale: 2.4,
			radians: 0.785398163, // 45 degrees in radians
			degrees: 45.,
			size: ViewportPosition::default(),
		}
	}
}

impl DocumentTransform {
	pub fn transform_string(&self) -> String {
		let inverse_scale = 1. / self.scale;
		let size = DocumentPosition {
			x: self.size.x as f64 / 2.,
			y: self.size.y as f64 / 2.,
		};
		let translation = (self.location * -inverse_scale).rotate(-self.radians);
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
	pub fn to_document_position(&self, document_transform: &DocumentTransform, apply_rotation: bool) -> DocumentPosition {
		if apply_rotation {
			DocumentPosition { x: self.x as f64, y: self.y as f64 }
				.add((document_transform.size.x as f64 * -0.5, document_transform.size.y as f64 * -0.5))
				.rotate(document_transform.radians)
				.mul(document_transform.scale)
				.add((document_transform.location.x, document_transform.location.y))
		} else {
			let (document_location_x, document_location_y): (f64, f64) = {
				let cosine = (-document_transform.radians).cos();
				let sine = (-document_transform.radians).sin();
				(
					document_transform.location.x * cosine - document_transform.location.y * sine,
					document_transform.location.x * sine + document_transform.location.y * cosine,
				)
			};
			DocumentPosition { x: self.x as f64, y: self.y as f64 }
				.add((document_transform.size.x as f64 * -0.5, document_transform.size.y as f64 * -0.5))
				.mul(document_transform.scale)
				.add((document_location_x, document_location_y))
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
