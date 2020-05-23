#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct Color {
	pub r: f32,
	pub g: f32,
	pub b: f32,
	pub a: f32,
}

impl Color {
	pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
		Self { r, g, b, a }
	}

	pub const TRANSPARENT: Self = Color {
		r: 0.0,
		g: 0.0,
		b: 0.0,
		a: 0.0,
	};

	pub const BLACK: Self = Color {
		r: 0.0,
		g: 0.0,
		b: 0.0,
		a: 1.0,
	};

	pub const WHITE: Self = Color {
		r: 1.0,
		g: 1.0,
		b: 1.0,
		a: 1.0,
	};

	pub const RED: Self = Color {
		r: 1.0,
		g: 0.0,
		b: 0.0,
		a: 1.0,
	};

	pub const YELLOW: Self = Color {
		r: 1.0,
		g: 1.0,
		b: 0.0,
		a: 1.0,
	};

	pub const GREEN: Self = Color {
		r: 0.0,
		g: 1.0,
		b: 0.0,
		a: 1.0,
	};

	pub const CYAN: Self = Color {
		r: 0.0,
		g: 1.0,
		b: 1.0,
		a: 1.0,
	};

	pub const BLUE: Self = Color {
		r: 0.0,
		g: 0.0,
		b: 1.0,
		a: 1.0,
	};

	pub const MAGENTA: Self = Color {
		r: 1.0,
		g: 0.0,
		b: 1.0,
		a: 1.0,
	};
}