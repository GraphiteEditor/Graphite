use core_types::registry::types::Percentage;
use core_types::table::Table;
use core_types::{ATTR_BLEND_MODE, ATTR_CLIPPING_MASK, ATTR_OPACITY, ATTR_OPACITY_FILL, BlendMode, Color, Ctx};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, Raster};
use vector_types::GradientStops;

pub(crate) trait MultiplyAlpha {
	fn multiply_alpha(&mut self, factor: f64);
}

impl MultiplyAlpha for Color {
	fn multiply_alpha(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}

fn multiply_table_attribute<T>(table: &mut Table<T>, key: &str, factor: f64) {
	if let Some(values) = table.iter_attribute_values_mut::<f64>(key) {
		for v in values {
			*v *= factor;
		}
	} else {
		for v in table.iter_attribute_values_mut_or_default::<f64>(key) {
			*v = factor;
		}
	}
}

impl MultiplyAlpha for Table<Vector> {
	fn multiply_alpha(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY, factor);
	}
}
impl MultiplyAlpha for Table<Graphic> {
	fn multiply_alpha(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY, factor);
	}
}
impl MultiplyAlpha for Table<Raster<CPU>> {
	fn multiply_alpha(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY, factor);
	}
}
impl MultiplyAlpha for Table<Color> {
	fn multiply_alpha(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY, factor);
	}
}
impl MultiplyAlpha for Table<GradientStops> {
	fn multiply_alpha(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY, factor);
	}
}

pub(crate) trait MultiplyFill {
	fn multiply_fill(&mut self, factor: f64);
}
impl MultiplyFill for Color {
	fn multiply_fill(&mut self, factor: f64) {
		*self = Color::from_rgbaf32_unchecked(self.r(), self.g(), self.b(), (self.a() * factor as f32).clamp(0., 1.))
	}
}
impl MultiplyFill for Table<Vector> {
	fn multiply_fill(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY_FILL, factor);
	}
}
impl MultiplyFill for Table<Graphic> {
	fn multiply_fill(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY_FILL, factor);
	}
}
impl MultiplyFill for Table<Raster<CPU>> {
	fn multiply_fill(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY_FILL, factor);
	}
}
impl MultiplyFill for Table<Color> {
	fn multiply_fill(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY_FILL, factor);
	}
}
impl MultiplyFill for Table<GradientStops> {
	fn multiply_fill(&mut self, factor: f64) {
		multiply_table_attribute(self, ATTR_OPACITY_FILL, factor);
	}
}

trait SetBlendMode {
	fn set_blend_mode(&mut self, blend_mode: BlendMode);
}

fn set_table_blend_mode<T>(table: &mut Table<T>, blend_mode: BlendMode) {
	for v in table.iter_attribute_values_mut_or_default::<BlendMode>(ATTR_BLEND_MODE) {
		*v = blend_mode;
	}
}

impl SetBlendMode for Table<Vector> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		set_table_blend_mode(self, blend_mode);
	}
}
impl SetBlendMode for Table<Graphic> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		set_table_blend_mode(self, blend_mode);
	}
}
impl SetBlendMode for Table<Raster<CPU>> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		set_table_blend_mode(self, blend_mode);
	}
}
impl SetBlendMode for Table<Color> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		set_table_blend_mode(self, blend_mode);
	}
}
impl SetBlendMode for Table<GradientStops> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		set_table_blend_mode(self, blend_mode);
	}
}

trait SetClip {
	fn set_clip(&mut self, clip: bool);
}

fn set_table_clip<T>(table: &mut Table<T>, clip: bool) {
	for v in table.iter_attribute_values_mut_or_default::<bool>(ATTR_CLIPPING_MASK) {
		*v = clip;
	}
}

impl SetClip for Table<Vector> {
	fn set_clip(&mut self, clip: bool) {
		set_table_clip(self, clip);
	}
}
impl SetClip for Table<Graphic> {
	fn set_clip(&mut self, clip: bool) {
		set_table_clip(self, clip);
	}
}
impl SetClip for Table<Raster<CPU>> {
	fn set_clip(&mut self, clip: bool) {
		set_table_clip(self, clip);
	}
}
impl SetClip for Table<Color> {
	fn set_clip(&mut self, clip: bool) {
		set_table_clip(self, clip);
	}
}
impl SetClip for Table<GradientStops> {
	fn set_clip(&mut self, clip: bool) {
		set_table_clip(self, clip);
	}
}

/// Applies the blend mode to the input graphics. Setting this allows for customizing how overlapping content is composited together.
#[node_macro::node(category("Blending"))]
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
	// TODO: Find a way to make this apply once to the table's parent (i.e. its item in its parent table or TableRow<T>) rather than applying to each item in its own table, which produces the undesired result
	content.set_blend_mode(blend_mode);
	content
}

/// Modifies the opacity and/or fill of the input graphics by multiplying the existing values by these percentages.
/// Opacity affects the transparency of the content (together with anything above which is clipped to it).
/// Fill affects the transparency of the content itself, independent of any content clipped to it.
#[node_macro::node(category("Blending"))]
fn opacity<T: MultiplyAlpha + MultiplyFill>(
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
	/// Whether the *Opacity* property is enabled, multiplying the existing opacity by the chosen percentage.
	#[widget(ParsedWidgetOverride::Hidden)]
	#[default(true)]
	has_opacity: bool,
	/// How visible the content should be, including any content clipped to it.
	/// Ranges from the default of 100% (fully opaque) to 0% (fully transparent).
	#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]
	#[default(100.)]
	opacity: Percentage,
	/// Whether the *Fill* property is enabled, multiplying the existing fill by the chosen percentage.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_fill: bool,
	/// How visible the content should be, independent of any content clipped to it.
	/// Ranges from 0% (fully transparent) to the default of 100% (fully opaque).
	#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]
	#[default(100.)]
	fill: Percentage,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its item in its parent table or TableRow<T>) rather than applying to each item in its own table, which produces the undesired result
	if has_opacity {
		content.multiply_alpha(opacity / 100.);
	}
	if has_fill {
		content.multiply_fill(fill / 100.);
	}
	content
}

/// Sets whether the input graphics inherit the alpha of the content beneath them, "clipping" them to that content.
#[node_macro::node(category("Blending"))]
fn clipping_mask<T: SetClip>(
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
	/// Whether the content inherits the alpha of the content beneath it.
	clip: bool,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its item in its parent table or TableRow<T>) rather than applying to each item in its own table, which produces the undesired result
	content.set_clip(clip);
	content
}
