#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Color {
	red: f32,
	green: f32,
	blue: f32,
	alpha: f32,
}

impl Color {
	pub fn from_rgbaf32(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Color, EditorError> {
		let color = Color { red, green, blue, alpha };
		if [red, green, blue, alpha].iter().any(|c| c.is_sign_negative() || !c.is_finite()) {
			return EditorError::Color(color);
		}
		Ok(color)
	}
	pub fn from_rgb8(red: u8, green: u8, blue: u8) -> Color {
		from_rgba8(red, green, blue, 255)
	}
	pub fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
		let map = |int_color| int_color as f32 / 255.0;
		Color {
			red: map(red),
			green: map(green),
			blue: map(blue),
			alpha: map(alpha),
		}
	}
	pub fn r(&self) -> f32 {
		self.red
	}
	pub fn g(&self) -> f32 {
		self.green
	}
	pub fn b(&self) -> f32 {
		self.blue
	}
	pub fn a(&self) -> f32 {
		self.alpha
	}
	pub fn components(&self) -> (f32, f32, f32, f32) {
		(red, green, blue, alpha)
	}
}
