use core_types::attribute::{Attr, BlendMode as BlendModeAttr, ClippingMask, Opacity, OpacityFill};
use core_types::registry::types::Percentage;
use core_types::{BlendMode, Ctx};


/// Applies the blend mode to the input graphics. Setting this allows for customizing how overlapping content is composited together.
#[node_macro::node(category("Blending"))]
fn blend_mode<T>(
	_: impl Ctx,
	/// The layer stack that will be composited when rendering.
	(element, _content_blend_mode): (T, Attr<BlendModeAttr>),
	/// The choice of equation that controls how brightness and color blends between overlapping pixels.
	blend_mode: BlendMode,
) -> (T, Attr<BlendModeAttr>) {
	(element, Attr(blend_mode))
}

/// Modifies the opacity and/or fill of the input graphics by multiplying the existing values by these percentages.
/// Opacity affects the transparency of the content (together with anything above which is clipped to it).
/// Fill affects the transparency of the content itself, independent of any content clipped to it.
#[node_macro::node(category("Blending"))]
fn opacity<T>(
	_: impl Ctx,
	/// The layer stack that will be composited when rendering.
	(element, content_opacity, content_fill): (T, Attr<Opacity>, Attr<OpacityFill>),
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
) -> (T, Attr<Opacity>, Attr<OpacityFill>) {
	let opacity = match has_opacity {
		true => *content_opacity * (opacity / 100.),
		false => *content_opacity,
	};
	let fill = match has_fill {
		true => *content_fill * (fill / 100.),
		false => *content_fill,
	};
	(element, Attr(opacity), Attr(fill))
}

/// Sets whether the input graphics inherit the alpha of the content beneath them, "clipping" them to that content.
#[node_macro::node(category("Blending"))]
fn clipping_mask<T>(
	_: impl Ctx,
	/// The layer stack that will be composited when rendering.
	(element, _content_clip): (T, Attr<ClippingMask>),
	/// Whether the content inherits the alpha of the content beneath it.
	clip: bool,
) -> (T, Attr<ClippingMask>) {
	(element, Attr(clip))
}
