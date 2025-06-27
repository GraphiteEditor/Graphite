use dyn_any::DynAny;
use std::hash::Hash;

#[derive(Copy, Clone, Debug, PartialEq, DynAny, specta::Type, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AlphaBlending {
	pub blend_mode: BlendMode,
	pub opacity: f32,
	pub fill: f32,
	pub clip: bool,
}
impl Default for AlphaBlending {
	fn default() -> Self {
		Self::new()
	}
}
impl Hash for AlphaBlending {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.opacity.to_bits().hash(state);
		self.fill.to_bits().hash(state);
		self.blend_mode.hash(state);
		self.clip.hash(state);
	}
}
impl std::fmt::Display for AlphaBlending {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let round = |x: f32| (x * 1e3).round() / 1e3;
		write!(
			f,
			"Blend Mode: {} — Opacity: {}% — Fill: {}% — Clip: {}",
			self.blend_mode,
			round(self.opacity * 100.),
			round(self.fill * 100.),
			if self.clip { "Yes" } else { "No" }
		)
	}
}

impl AlphaBlending {
	pub const fn new() -> Self {
		Self {
			opacity: 1.,
			fill: 1.,
			blend_mode: BlendMode::Normal,
			clip: false,
		}
	}

	pub fn lerp(&self, other: &Self, t: f32) -> Self {
		let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

		AlphaBlending {
			opacity: lerp(self.opacity, other.opacity, t),
			fill: lerp(self.fill, other.fill, t),
			blend_mode: if t < 0.5 { self.blend_mode } else { other.blend_mode },
			clip: if t < 0.5 { self.clip } else { other.clip },
		}
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, DynAny, Hash, specta::Type)]
#[repr(i32)]
pub enum BlendMode {
	// Basic group
	#[default]
	Normal,

	// Darken group
	Darken,
	Multiply,
	ColorBurn,
	LinearBurn,
	DarkerColor,

	// Lighten group
	Lighten,
	Screen,
	ColorDodge,
	LinearDodge,
	LighterColor,

	// Contrast group
	Overlay,
	SoftLight,
	HardLight,
	VividLight,
	LinearLight,
	PinLight,
	HardMix,

	// Inversion group
	Difference,
	Exclusion,
	Subtract,
	Divide,

	// Component group
	Hue,
	Saturation,
	Color,
	Luminosity,

	// Other stuff
	Erase,
	Restore,
	MultiplyAlpha,
}

impl BlendMode {
	/// All standard blend modes ordered by group.
	pub fn list() -> [&'static [BlendMode]; 6] {
		use BlendMode::*;
		[
			// Normal group
			&[Normal],
			// Darken group
			&[Darken, Multiply, ColorBurn, LinearBurn, DarkerColor],
			// Lighten group
			&[Lighten, Screen, ColorDodge, LinearDodge, LighterColor],
			// Contrast group
			&[Overlay, SoftLight, HardLight, VividLight, LinearLight, PinLight, HardMix],
			// Inversion group
			&[Difference, Exclusion, Subtract, Divide],
			// Component group
			&[Hue, Saturation, Color, Luminosity],
		]
	}

	/// The subset of [`BlendMode::list()`] that is supported by SVG.
	pub fn list_svg_subset() -> [&'static [BlendMode]; 6] {
		use BlendMode::*;
		[
			// Normal group
			&[Normal],
			// Darken group
			&[Darken, Multiply, ColorBurn],
			// Lighten group
			&[Lighten, Screen, ColorDodge],
			// Contrast group
			&[Overlay, SoftLight, HardLight],
			// Inversion group
			&[Difference, Exclusion],
			// Component group
			&[Hue, Saturation, Color, Luminosity],
		]
	}

	pub fn index_in_list(&self) -> Option<usize> {
		Self::list().iter().flat_map(|x| x.iter()).position(|&blend_mode| blend_mode == *self)
	}

	pub fn index_in_list_svg_subset(&self) -> Option<usize> {
		Self::list_svg_subset().iter().flat_map(|x| x.iter()).position(|&blend_mode| blend_mode == *self)
	}

	/// Convert the enum to the CSS string for the blend mode.
	/// [Read more](https://developer.mozilla.org/en-US/docs/Web/CSS/blend-mode#values)
	pub fn to_svg_style_name(&self) -> Option<&'static str> {
		match self {
			// Normal group
			BlendMode::Normal => Some("normal"),
			// Darken group
			BlendMode::Darken => Some("darken"),
			BlendMode::Multiply => Some("multiply"),
			BlendMode::ColorBurn => Some("color-burn"),
			// Lighten group
			BlendMode::Lighten => Some("lighten"),
			BlendMode::Screen => Some("screen"),
			BlendMode::ColorDodge => Some("color-dodge"),
			// Contrast group
			BlendMode::Overlay => Some("overlay"),
			BlendMode::SoftLight => Some("soft-light"),
			BlendMode::HardLight => Some("hard-light"),
			// Inversion group
			BlendMode::Difference => Some("difference"),
			BlendMode::Exclusion => Some("exclusion"),
			// Component group
			BlendMode::Hue => Some("hue"),
			BlendMode::Saturation => Some("saturation"),
			BlendMode::Color => Some("color"),
			BlendMode::Luminosity => Some("luminosity"),
			_ => None,
		}
	}

	/// Renders the blend mode CSS style declaration.
	pub fn render(&self) -> String {
		format!(
			r#" mix-blend-mode: {};"#,
			self.to_svg_style_name().unwrap_or_else(|| {
				warn!("Unsupported blend mode {self:?}");
				"normal"
			})
		)
	}
}

impl std::fmt::Display for BlendMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			// Normal group
			BlendMode::Normal => write!(f, "Normal"),
			// Darken group
			BlendMode::Darken => write!(f, "Darken"),
			BlendMode::Multiply => write!(f, "Multiply"),
			BlendMode::ColorBurn => write!(f, "Color Burn"),
			BlendMode::LinearBurn => write!(f, "Linear Burn"),
			BlendMode::DarkerColor => write!(f, "Darker Color"),
			// Lighten group
			BlendMode::Lighten => write!(f, "Lighten"),
			BlendMode::Screen => write!(f, "Screen"),
			BlendMode::ColorDodge => write!(f, "Color Dodge"),
			BlendMode::LinearDodge => write!(f, "Linear Dodge"),
			BlendMode::LighterColor => write!(f, "Lighter Color"),
			// Contrast group
			BlendMode::Overlay => write!(f, "Overlay"),
			BlendMode::SoftLight => write!(f, "Soft Light"),
			BlendMode::HardLight => write!(f, "Hard Light"),
			BlendMode::VividLight => write!(f, "Vivid Light"),
			BlendMode::LinearLight => write!(f, "Linear Light"),
			BlendMode::PinLight => write!(f, "Pin Light"),
			BlendMode::HardMix => write!(f, "Hard Mix"),
			// Inversion group
			BlendMode::Difference => write!(f, "Difference"),
			BlendMode::Exclusion => write!(f, "Exclusion"),
			BlendMode::Subtract => write!(f, "Subtract"),
			BlendMode::Divide => write!(f, "Divide"),
			// Component group
			BlendMode::Hue => write!(f, "Hue"),
			BlendMode::Saturation => write!(f, "Saturation"),
			BlendMode::Color => write!(f, "Color"),
			BlendMode::Luminosity => write!(f, "Luminosity"),
			// Other utility blend modes (hidden from the normal list)
			BlendMode::Erase => write!(f, "Erase"),
			BlendMode::Restore => write!(f, "Restore"),
			BlendMode::MultiplyAlpha => write!(f, "Multiply Alpha"),
		}
	}
}

#[cfg(feature = "vello")]
impl From<BlendMode> for vello::peniko::Mix {
	fn from(val: BlendMode) -> Self {
		match val {
			// Normal group
			BlendMode::Normal => vello::peniko::Mix::Normal,
			// Darken group
			BlendMode::Darken => vello::peniko::Mix::Darken,
			BlendMode::Multiply => vello::peniko::Mix::Multiply,
			BlendMode::ColorBurn => vello::peniko::Mix::ColorBurn,
			// Lighten group
			BlendMode::Lighten => vello::peniko::Mix::Lighten,
			BlendMode::Screen => vello::peniko::Mix::Screen,
			BlendMode::ColorDodge => vello::peniko::Mix::ColorDodge,
			// Contrast group
			BlendMode::Overlay => vello::peniko::Mix::Overlay,
			BlendMode::SoftLight => vello::peniko::Mix::SoftLight,
			BlendMode::HardLight => vello::peniko::Mix::HardLight,
			// Inversion group
			BlendMode::Difference => vello::peniko::Mix::Difference,
			BlendMode::Exclusion => vello::peniko::Mix::Exclusion,
			// Component group
			BlendMode::Hue => vello::peniko::Mix::Hue,
			BlendMode::Saturation => vello::peniko::Mix::Saturation,
			BlendMode::Color => vello::peniko::Mix::Color,
			BlendMode::Luminosity => vello::peniko::Mix::Luminosity,
			_ => todo!(),
		}
	}
}
