use core_types::list::List;
use core_types::{
	ATTR_EDITOR_LAYER_PATH, ATTR_FONT_SIZE, ATTR_TEXT_ALIGN, ATTR_TEXT_CHARACTER_SPACING, ATTR_TEXT_FONT, ATTR_TEXT_LINE_HEIGHT, ATTR_TEXT_MAX_HEIGHT, ATTR_TEXT_MAX_WIDTH, ATTR_TEXT_TILT,
	ATTR_TRANSFORM, Ctx,
};
use graph_craft::application_io::resource::Resource;
use graphic_types::Vector;
pub use text_nodes::*;

/// Draws a text string as vector geometry with a choice of font and styling.
#[node_macro::node(category("Text"))]
fn text(
	_: impl Ctx,
	_primary: (),
	/// The text content to be drawn.
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("Lorem ipsum")]
	text: String,
	/// The loaded font file used to draw the text. The editor resolves the chosen typeface to these bytes via the resource system.
	#[widget(ParsedWidgetOverride::Custom = "text_font")]
	font: Resource,
	/// The font size used to draw the text.
	#[unit(" px")]
	#[default(24.)]
	#[hard_min(1.)]
	size: f64,
	/// The line height ratio, relative to the font size. Each line is drawn lower than its previous line by the distance of *Size* × *Line Height*.
	///
	/// 0 means all lines overlap. 1 means all lines are spaced by just the font size. 1.2 is a common default for readable text. 2 means double-spaced text.
	#[unit("x")]
	#[hard_min(0.)]
	#[step(0.1)]
	#[default(1.2)]
	line_height: f64,
	/// Additional spacing, in pixels, added between each character.
	#[unit(" px")]
	#[step(0.1)]
	character_spacing: f64,
	/// Whether the *Max Width* property is enabled so that lines can wrap to fit its specified block width.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_width: bool,
	/// The maximum width that the text block can occupy before wrapping to a new line. Otherwise, lines do not wrap.
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_width: f64,
	/// Whether the *Max Height* property is enabled so that lines beyond it are not drawn.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_height: bool,
	/// The maximum height that the text block can occupy. Excess lines are not drawn.
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_height: f64,
	/// The angle of faux italic slant applied to each glyph.
	#[unit("°")]
	#[hard_min(-85.)]
	#[hard_max(85.)]
	tilt: f64,
	/// The horizontal alignment of each line of text within its surrounding box.
	/// To have an effect on a single line of text, *Max Width* must be set.
	#[widget(ParsedWidgetOverride::Custom = "text_align")]
	align: TextAlign,
	/// Whether to split every letterform into its own vector item. Otherwise, a single vector compound path is produced.
	separate_glyphs: bool,
) -> List<Vector> {
	let typesetting = TypesettingConfig {
		font_size: size,
		line_height_ratio: line_height,
		character_spacing,
		max_width: has_max_width.then_some(max_width),
		max_height: has_max_height.then_some(max_height),
		tilt,
		align,
	};

	to_path(&text, &font, typesetting, separate_glyphs)
}

/// Produces a styled `List<String>` carrying all typographic attributes.
#[node_macro::node(category("Text"))]
fn text_layer(
	_: impl Ctx,
	_primary: (),
	/// The text content to display.
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("Lorem ipsum")]
	text: String,
	/// The loaded font file used to render the text. The editor resolves the chosen typeface to these bytes via the resource system.
	#[widget(ParsedWidgetOverride::Custom = "text_font")]
	font: Resource,
	/// Font size in document-space pixels.
	#[unit(" px")]
	#[default(24.)]
	#[hard_min(1.)]
	size: f64,
	/// Line height ratio relative to the font size. 1.2 is the typical default for body copy.
	#[unit("x")]
	#[hard_min(0.)]
	#[step(0.1)]
	#[default(1.2)]
	line_height: f64,
	/// Additional spacing in document-space pixels added between every character pair.
	#[unit(" px")]
	#[step(0.1)]
	character_spacing: f64,
	/// Enables the maximum width constraint so lines can wrap.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_width: bool,
	/// Maximum line-wrap width in document-space pixels.
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_width: f64,
	/// Enables the maximum height constraint so excess lines are clipped.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_height: bool,
	/// Maximum block height in document-space pixels; lines whose baseline exceeds this are not drawn.
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_height: f64,
	/// Faux-italic slant angle in degrees.
	#[unit("°")]
	#[hard_min(-85.)]
	#[hard_max(85.)]
	tilt: f64,
	/// Horizontal alignment of each line within the text block.
	#[widget(ParsedWidgetOverride::Custom = "text_align")]
	align: TextAlign,
) -> List<String> {
	const DEFAULT_FONT_SIZE: f64 = 24.;
	const DEFAULT_LINE_HEIGHT: f64 = 1.2;

	let mut list = List::new_from_element(text);

	// Insert only when value deviates from its default as each stored attribute has runtime cost.

	if font != Resource::default() {
		list.set_attribute(ATTR_TEXT_FONT, 0, font);
	}
	if (size - DEFAULT_FONT_SIZE).abs() > f64::EPSILON {
		list.set_attribute(ATTR_FONT_SIZE, 0, size);
	}
	if (line_height - DEFAULT_LINE_HEIGHT).abs() > f64::EPSILON {
		list.set_attribute(ATTR_TEXT_LINE_HEIGHT, 0, line_height);
	}
	if character_spacing != 0. {
		list.set_attribute(ATTR_TEXT_CHARACTER_SPACING, 0, character_spacing);
	}
	if has_max_width {
		list.set_attribute(ATTR_TEXT_MAX_WIDTH, 0, Some(max_width));
	}
	if has_max_height {
		list.set_attribute(ATTR_TEXT_MAX_HEIGHT, 0, Some(max_height));
	}
	if tilt != 0. {
		list.set_attribute(ATTR_TEXT_TILT, 0, tilt);
	}
	if align != TextAlign::default() {
		list.set_attribute(ATTR_TEXT_ALIGN, 0, align);
	}

	list
}

/// Converts a styled `List<String>` into vector geometry.
/// Each string item is independently shaped by Parley and vectorised via skrifa.
#[node_macro::node(category("Text"))]
fn text_to_vector(
	_: impl Ctx,
	/// A styled list of text strings produced by the **Text Layer** node (or any other `List<String>` source).
	#[implementations(List<String>)]
	strings: List<String>,
	/// When enabled, each glyph is emitted as its own vector item instead of a single compound path per string.
	separate_glyphs: bool,
) -> List<Vector> {
	let mut result = List::new();

	for index in 0..strings.len() {
		let Some(text) = strings.element(index) else { continue };
		if text.is_empty() {
			continue;
		}

		let font: Resource = strings.attribute_cloned_or_default(ATTR_TEXT_FONT, index);

		let typesetting = TypesettingConfig {
			font_size: strings.attribute_cloned_or(ATTR_FONT_SIZE, index, 24.),
			line_height_ratio: strings.attribute_cloned_or(ATTR_TEXT_LINE_HEIGHT, index, 1.2),
			character_spacing: strings.attribute_cloned_or(ATTR_TEXT_CHARACTER_SPACING, index, 0.),
			max_width: strings.attribute_cloned_or::<Option<f64>>(ATTR_TEXT_MAX_WIDTH, index, None),
			max_height: strings.attribute_cloned_or::<Option<f64>>(ATTR_TEXT_MAX_HEIGHT, index, None),
			tilt: strings.attribute_cloned_or(ATTR_TEXT_TILT, index, 0.),
			align: strings.attribute_cloned_or(ATTR_TEXT_ALIGN, index, TextAlign::default()),
		};

		let vectors = to_path(text, &font, typesetting, separate_glyphs);
		let transform = strings.attribute_cloned_or_default::<glam::DAffine2>(ATTR_TRANSFORM, index);
		let layer_path = strings.attribute_cloned_or_default::<List<graph_craft::document::NodeId>>(ATTR_EDITOR_LAYER_PATH, index);

		for mut item in vectors.into_iter() {
			if transform != glam::DAffine2::IDENTITY {
				let local = item.attribute_cloned_or_default::<glam::DAffine2>(ATTR_TRANSFORM);
				item.set_attribute(ATTR_TRANSFORM, transform * local);
			}
			if !layer_path.is_empty() {
				item.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.clone());
			}
			result.push(item);
		}
	}

	result
}
