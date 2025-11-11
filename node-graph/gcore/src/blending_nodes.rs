use crate::gradient::GradientStops;
use crate::raster_types::{CPU, Raster};
use crate::registry::types::Percentage;
use crate::table::Table;
use crate::vector::Vector;
use crate::{BlendMode, Color, Ctx, Graphic};

pub(super) trait MultiplyAlpha {
	fn multiply_alpha(&mut self, factor: f64);
}

impl MultiplyAlpha for Color {
	fn multiply_alpha(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}
impl MultiplyAlpha for Table<Vector> {
	fn multiply_alpha(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.opacity *= factor as f32;
		}
	}
}
impl MultiplyAlpha for Table<Graphic> {
	fn multiply_alpha(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.opacity *= factor as f32;
		}
	}
}
impl MultiplyAlpha for Table<Raster<CPU>> {
	fn multiply_alpha(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.opacity *= factor as f32;
		}
	}
}
impl MultiplyAlpha for Table<Color> {
	fn multiply_alpha(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.opacity *= factor as f32;
		}
	}
}
impl MultiplyAlpha for Table<GradientStops> {
	fn multiply_alpha(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.opacity *= factor as f32;
		}
	}
}

pub(super) trait MultiplyFill {
	fn multiply_fill(&mut self, factor: f64);
}
impl MultiplyFill for Color {
	fn multiply_fill(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}
impl MultiplyFill for Table<Vector> {
	fn multiply_fill(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.fill *= factor as f32;
		}
	}
}
impl MultiplyFill for Table<Graphic> {
	fn multiply_fill(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.fill *= factor as f32;
		}
	}
}
impl MultiplyFill for Table<Raster<CPU>> {
	fn multiply_fill(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.fill *= factor as f32;
		}
	}
}
impl MultiplyFill for Table<Color> {
	fn multiply_fill(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.fill *= factor as f32;
		}
	}
}
impl MultiplyFill for Table<GradientStops> {
	fn multiply_fill(&mut self, factor: f64) {
		for row in self.iter_mut() {
			row.alpha_blending.fill *= factor as f32;
		}
	}
}

trait SetBlendMode {
	fn set_blend_mode(&mut self, blend_mode: BlendMode);
}

impl SetBlendMode for Table<Vector> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for row in self.iter_mut() {
			row.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for Table<Graphic> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for row in self.iter_mut() {
			row.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for Table<Raster<CPU>> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for row in self.iter_mut() {
			row.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for Table<Color> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for row in self.iter_mut() {
			row.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for Table<GradientStops> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for row in self.iter_mut() {
			row.alpha_blending.blend_mode = blend_mode;
		}
	}
}

trait SetClip {
	fn set_clip(&mut self, clip: bool);
}

impl SetClip for Table<Vector> {
	fn set_clip(&mut self, clip: bool) {
		for row in self.iter_mut() {
			row.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for Table<Graphic> {
	fn set_clip(&mut self, clip: bool) {
		for row in self.iter_mut() {
			row.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for Table<Raster<CPU>> {
	fn set_clip(&mut self, clip: bool) {
		for row in self.iter_mut() {
			row.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for Table<Color> {
	fn set_clip(&mut self, clip: bool) {
		for row in self.iter_mut() {
			row.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for Table<GradientStops> {
	fn set_clip(&mut self, clip: bool) {
		for row in self.iter_mut() {
			row.alpha_blending.clip = clip;
		}
	}
}

/// Applies the blend mode to the input graphics. Setting this allows for customizing how overlapping content is composited together.
#[node_macro::node(category("Style"))]
fn blend_mode<T: SetBlendMode>(
	_: impl Ctx,
	/// The layer stack that will be composited when rendering.
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	mut content: T,
	/// The choice of equation that controls how brightness and color blends between overlapping pixels.
	blend_mode: BlendMode,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or TableRow<T>) rather than applying to each row in its own table, which produces the undesired result
	content.set_blend_mode(blend_mode);
	content
}

/// Modifies the opacity of the input graphics by multiplying the existing opacity by this percentage.
/// This affects the transparency of the content (together with anything above which is clipped to it).
#[node_macro::node(category("Style"))]
fn opacity<T: MultiplyAlpha>(
	_: impl Ctx,
	/// The layer stack that will be composited when rendering.
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	mut content: T,
	/// How visible the content should be, including any content clipped to it.
	/// Ranges from the default of 100% (fully opaque) to 0% (fully transparent).
	#[default(100.)]
	opacity: Percentage,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or TableRow<T>) rather than applying to each row in its own table, which produces the undesired result
	content.multiply_alpha(opacity / 100.);
	content
}

/// Sets each of the blending properties at once. The blend mode determines how overlapping content is composited together. The opacity affects the transparency of the content (together with anything above which is clipped to it). The fill affects the transparency of the content itself, without affecting that of content clipped to it. The clip property determines whether the content inherits the alpha of the content beneath it.
#[node_macro::node(category("Style"))]
fn blending<T: SetBlendMode + MultiplyAlpha + MultiplyFill + SetClip>(
	_: impl Ctx,
	/// The layer stack that will be composited when rendering.
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	mut content: T,
	/// The choice of equation that controls how brightness and color blends between overlapping pixels.
	blend_mode: BlendMode,
	/// How visible the content should be, including any content clipped to it.
	/// Ranges from the default of 100% (fully opaque) to 0% (fully transparent).
	#[default(100.)]
	opacity: Percentage,
	/// How visible the content should be, independent of any content clipped to it.
	/// Ranges from 0% (fully transparent) to 100% (fully opaque).
	#[default(100.)]
	fill: Percentage,
	/// Whether the content inherits the alpha of the content beneath it.
	#[default(false)]
	clip: bool,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or TableRow<T>) rather than applying to each row in its own table, which produces the undesired result
	content.set_blend_mode(blend_mode);
	content.multiply_alpha(opacity / 100.);
	content.multiply_fill(fill / 100.);
	content.set_clip(clip);
	content
}
