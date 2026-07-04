use core_types::list::Item;
use core_types::registry::types::Percentage;
use core_types::{ATTR_BLEND_MODE, ATTR_CLIPPING_MASK, ATTR_OPACITY, ATTR_OPACITY_FILL, BlendMode, Color, Ctx};
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, Raster};
use vector_types::GradientStops;

/// Applies the blend mode to the input graphics. Setting this allows for customizing how overlapping content is composited together.
#[node_macro::node(category("Blending"))]
fn blend_mode<T>(
	_: impl Ctx,
	/// The content that will be composited when rendering.
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)]
	mut content: Item<T>,
	/// The choice of equation that controls how brightness and color blends between overlapping pixels.
	blend_mode: BlendMode,
) -> Item<T> {
	content.set_attribute(ATTR_BLEND_MODE, blend_mode);
	content
}

/// Modifies the opacity and/or fill of the input graphics by multiplying the existing values by these percentages.
/// Opacity affects the transparency of the content (together with anything above which is clipped to it).
/// Fill affects the transparency of the content itself, independent of any content clipped to it.
#[node_macro::node(category("Blending"))]
fn opacity<T>(
	_: impl Ctx,
	/// The content that will be composited when rendering.
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)]
	mut content: Item<T>,
	/// Whether the *Opacity* property is enabled, multiplying the existing opacity by the chosen percentage.
	#[widget(ParsedWidgetOverride::Hidden)]
	#[default(true)]
	has_opacity: Item<bool>,
	/// How visible the content should be, including any content clipped to it.
	/// Ranges from the default of 100% (fully opaque) to 0% (fully transparent).
	#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]
	#[default(100.)]
	opacity: Item<Percentage>,
	/// Whether the *Fill* property is enabled, multiplying the existing fill by the chosen percentage.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_fill: Item<bool>,
	/// How visible the content should be, independent of any content clipped to it.
	/// Ranges from 0% (fully transparent) to the default of 100% (fully opaque).
	#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]
	#[default(100.)]
	fill: Item<Percentage>,
) -> Item<T> {
	let (has_opacity, opacity, has_fill, fill) = (*has_opacity.element(), *opacity.element(), *has_fill.element(), *fill.element());

	if has_opacity {
		let multiplied = content.attribute_cloned_or(ATTR_OPACITY, 1.) * (opacity / 100.);
		content.set_attribute(ATTR_OPACITY, multiplied);
	}

	if has_fill {
		let multiplied = content.attribute_cloned_or(ATTR_OPACITY_FILL, 1.) * (fill / 100.);
		content.set_attribute(ATTR_OPACITY_FILL, multiplied);
	}

	content
}

/// Sets whether the input graphics inherit the alpha of the content beneath them, "clipping" them to that content.
#[node_macro::node(category("Blending"))]
fn clipping_mask<T>(
	_: impl Ctx,
	/// The content that will be composited when rendering.
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)]
	mut content: Item<T>,
	/// Whether the content inherits the alpha of the content beneath it.
	clip: Item<bool>,
) -> Item<T> {
	let clip = *clip.element();

	content.set_attribute(ATTR_CLIPPING_MASK, clip);
	content
}
