#[allow(dead_code)]
pub enum ColorPalette {
	Black,
	NearBlack,
	MildBlack,
	DarkGray,
	DimGray,
	DullGray,
	LowerGray,
	MiddleGray,
	UpperGray,
	PaleGray,
	SoftGray,
	LightGray,
	BrightGray,
	MildWhite,
	NearWhite,
	White,
	Accent,
}

impl ColorPalette {
	pub fn get_color(self) -> wgpu::Color {
		let grayscale = match self {
			ColorPalette::Black => 0 * 17, // #000000
			ColorPalette::NearBlack => 1 * 17, // #111111
			ColorPalette::MildBlack => 2 * 17, // #222222
			ColorPalette::DarkGray => 3 * 17, // #333333
			ColorPalette::DimGray => 4 * 17, // #444444
			ColorPalette::DullGray => 5 * 17, // #555555
			ColorPalette::LowerGray => 6 * 17, // #666666
			ColorPalette::MiddleGray => 7 * 17, // #777777
			ColorPalette::UpperGray => 8 * 17, // #888888
			ColorPalette::PaleGray => 9 * 17, // #999999
			ColorPalette::SoftGray => 10 * 17, // #aaaaaa
			ColorPalette::LightGray => 11 * 17, // #bbbbbb
			ColorPalette::BrightGray => 12 * 17, // #cccccc
			ColorPalette::MildWhite => 13 * 17, // #dddddd
			ColorPalette::NearWhite => 14 * 17, // #eeeeee
			ColorPalette::White => 15 * 17, // #ffffff
			_ => -1,
		};

		if grayscale > -1 {
			let value = grayscale as f64 / 255.0;
			return wgpu::Color { r: value, g: value, b: value, a: 1.0 };
		}

		let rgba = match self {
			ColorPalette::Accent => (75, 121, 167, 255), // #4b79a7
			_ => (0, 0, 0, 255), // Unimplemented returns black
		};

		wgpu::Color {
			r: rgba.0 as f64 / 255.0,
			g: rgba.1 as f64 / 255.0,
			b: rgba.2 as f64 / 255.0,
			a: rgba.3 as f64 / 255.0
		}
	}

	pub fn get_color_linear(self) -> wgpu::Color {
		let standard_rgb = ColorPalette::get_color(self);

		let linear = palette::Srgb::new(standard_rgb.r, standard_rgb.g, standard_rgb.b).into_linear();

		wgpu::Color { r: linear.red, g: linear.green, b: linear.blue, a: standard_rgb.a }
	}
}