use core_types::consts::{DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT};
use core_types::list::{Item, List};
use core_types::{ATTR_FONT, ATTR_FONT_SIZE, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_TEXT_ALIGN, ATTR_TRANSFORM, Ctx};
use glam::DAffine2;
use graph_craft::application_io::resource::Resource;
use graphic_types::Vector;
pub use text_nodes::*;

/// Produces a styled text string carrying all typographic attributes.
///
/// Use the **Text to Vector** node to convert this into vector geometry if desired.
#[node_macro::node(category("Text"))]
fn text(
	_: impl Ctx,
	_primary: (),
	/// The text content to be drawn.
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("Lorem ipsum")]
	text: Item<String>,
	/// The loaded font file used to draw the text. The editor resolves the chosen typeface to these bytes via the resource system.
	#[widget(ParsedWidgetOverride::Custom = "text_font")]
	font: Item<Resource>,
	/// The font size used to draw the text.
	#[unit(" px")]
	#[default(24.)]
	#[hard(1..)]
	size: Item<f64>,
	/// The line height ratio, relative to the font size. Each line is drawn lower than its previous line by the distance of *Size* × *Line Height*.
	///
	/// 0 means all lines overlap. 1 means all lines are spaced by just the font size. 1.2 is a common default for readable text. 2 means double-spaced text.
	#[unit("x")]
	#[hard(0..)]
	#[soft(..2)]
	#[step(0.1)]
	#[default(1.2)]
	line_height: Item<f64>,
	/// Additional spacing, in pixels, added between each character.
	#[unit(" px")]
	#[step(0.1)]
	letter_spacing: Item<f64>,
	/// The angle of faux italic slant applied to each glyph.
	#[unit("°")]
	#[range]
	#[hard(-85..85)]
	letter_tilt: Item<f64>,
	/// Enables the maximum width constraint so lines can wrap.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_width: Item<bool>,
	/// The maximum width that the text block can occupy before wrapping to a new line. Otherwise, lines do not wrap.
	#[unit(" px")]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_width: Item<f64>,
	/// Whether the *Max Height* property is enabled so that lines beyond it are not drawn.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_height: Item<bool>,
	/// The maximum height that the text block can occupy. Excess lines are not drawn.
	#[unit(" px")]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_height: Item<f64>,
	/// The horizontal alignment of each line of text within its surrounding box. To have an effect on a single line of text, *Max Width* must be set.
	#[widget(ParsedWidgetOverride::Custom = "text_align")]
	align: Item<TextAlign>,
) -> Item<String> {
	let text = text.into_element();
	let font = font.into_element();
	let (size, line_height, letter_spacing, letter_tilt) = (*size.element(), *line_height.element(), *letter_spacing.element(), *letter_tilt.element());
	let (has_max_width, max_width, has_max_height, max_height) = (*has_max_width.element(), *max_width.element(), *has_max_height.element(), *max_height.element());
	let align = align.into_element();

	let mut item = Item::new_from_element(text);

	if font != Resource::default() {
		item.set_attribute(ATTR_FONT, font);
	}
	if (size - DEFAULT_FONT_SIZE).abs() > f64::EPSILON {
		item.set_attribute(ATTR_FONT_SIZE, size);
	}
	if (line_height - DEFAULT_LINE_HEIGHT).abs() > f64::EPSILON {
		item.set_attribute(ATTR_LINE_HEIGHT, line_height);
	}
	if letter_spacing != 0. {
		item.set_attribute(ATTR_LETTER_SPACING, letter_spacing);
	}
	if letter_tilt != 0. {
		item.set_attribute(ATTR_LETTER_TILT, letter_tilt);
	}
	if has_max_width {
		item.set_attribute(ATTR_MAX_WIDTH, max_width);
	}
	if has_max_height {
		item.set_attribute(ATTR_MAX_HEIGHT, max_height);
	}
	if align != TextAlign::default() {
		item.set_attribute(ATTR_TEXT_ALIGN, align);
	}

	item
}

/// Converts a styled text string into a vector compound path.
#[node_macro::node(category("Text"), name("Text to Vector"))]
fn text_to_vector(
	_: impl Ctx,
	/// A styled text string produced by the **Text** node (or any other string source).
	string: Item<String>,
) -> Item<Vector> {
	shape_text_item(&string, false).into_iter().next().unwrap_or_default()
}

/// Splits a styled text string into a separate vector item for each of its glyphs (letterforms).
#[node_macro::node(category("Text"), name("Text to Vector Glyphs"))]
fn text_to_vector_glyphs(
	_: impl Ctx,
	/// A styled text string produced by the **Text** node (or any other string source).
	string: Item<String>,
) -> List<Vector> {
	shape_text_item(&string, true)
}

/// Treats an unset or non-positive optional length input as absent.
fn optional_length(enabled: bool, value: f64) -> Option<f64> {
	(enabled && value.is_finite() && value > 0.).then_some(value)
}

/// Flows a styled text string along a path, following the SVG 2 text-on-path layout rules.
///
/// Takes the styled string the **Text** node produces rather than re-declaring font and typesetting inputs, so text on a
/// path stays in step with the rest of the text pipeline.
#[node_macro::node(category("Text"), name("Text on Path"))]
fn text_on_path(
	_: impl Ctx,
	/// A styled text string produced by the **Text** node (or any other string source).
	string: Item<String>,
	/// The path whose glyphs follow.
	path: Item<Vector>,
	/// Which side of the path's direction the glyphs sit on.
	#[default(TextPathSide::Left)]
	side: Item<TextPathSide>,
	/// Where the text starts relative to its position along the path.
	#[default(TextAnchor::Start)]
	text_anchor: Item<TextAnchor>,
	/// Distance from the path's start to the first glyph, as a percentage of the path's length. Negative values and
	/// values past 100% are allowed, so text can start before the path or past its end.
	#[unit("%")]
	#[default(0.)]
	start_offset: Item<f64>,
	/// Whether glyphs keep their size along the path or stretch to follow its curve.
	#[default(TextPathMethod::Align)]
	method: Item<TextPathMethod>,
	/// Whether the *Text Length* target is enabled so the text is fitted to it.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_text_length: Item<bool>,
	/// Target length for the text along the path.
	#[unit(" px")]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	#[hard(1..)]
	text_length: Item<f64>,
	/// How the target length is reached: by spacing alone, or by scaling glyphs too.
	#[default(LengthAdjust::Spacing)]
	length_adjust: Item<LengthAdjust>,
) -> List<Vector> {
	let defaults = TypesettingConfig::default();
	let text = string.element().clone();

	let font: Resource = {
		let font: Resource = string.attribute_cloned_or_default(ATTR_FONT);
		if font.is_empty() { FALLBACK_FONT_RESOURCE.clone() } else { font }
	};

	let typesetting = TypesettingConfig {
		font_size: string.attribute_cloned_or(ATTR_FONT_SIZE, defaults.font_size),
		letter_spacing: string.attribute_cloned_or(ATTR_LETTER_SPACING, defaults.letter_spacing),
		letter_tilt: string.attribute_cloned_or(ATTR_LETTER_TILT, defaults.letter_tilt),
		..defaults
	};

	let start_offset = start_offset.into_element();
	let text_anchor = text_anchor.into_element();

	let side = side.into_element();
	let method = method.into_element();
	let length_adjust = length_adjust.into_element();

	// Glyphs are placed in the styled string's own space, so the string's transform rides on each produced path.
	// The path's own transform is baked in before measuring, or a moved path lays its text out at the origin.
	let transform: DAffine2 = string.attribute_cloned_or_default(ATTR_TRANSFORM);
	let path_transform: DAffine2 = path.attribute_cloned_or_default(ATTR_TRANSFORM);

	let mut placed = place_text_on_path(
		&text,
		&core_types::list::List::new_from_element(path.into_element()),
		path_transform,
		&font,
		typesetting.font_size,
		typesetting.letter_spacing,
		start_offset,
		side,
		text_anchor,
		method,
		optional_length(has_text_length.into_element(), text_length.into_element()),
		length_adjust,
	);
	if transform != DAffine2::IDENTITY {
		// Glyphs are placed in the styled string's own space, so its transform rides on each produced item.
		for index in 0..placed.len() {
			let local = placed.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index);
			placed.set_attribute(ATTR_TRANSFORM, index, transform * local);
		}
	}
	placed
}
