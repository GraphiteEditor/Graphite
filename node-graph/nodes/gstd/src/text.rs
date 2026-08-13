use core_types::consts::{DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT};
use core_types::list::{Item, List};
use core_types::{ATTR_FONT, ATTR_FONT_SIZE, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_TEXT_ALIGN, Ctx};
use graph_craft::application_io::resource::Resource;
use graphic_types::Vector;
pub use text_nodes::text_on_path::{LengthAdjust, TextAnchor, TextPathMethod, TextPathSide, TextPathSpacing};
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
	#[step(0.1)]
	#[default(1.2)]
	line_height: Item<f64>,
	/// Additional spacing, in pixels, added between each character.
	#[unit(" px")]
	#[step(0.1)]
	letter_spacing: Item<f64>,
	/// The angle of faux italic slant applied to each glyph.
	#[unit("°")]
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
		item.set_attribute(ATTR_MAX_WIDTH, Some(max_width));
	}
	if has_max_height {
		item.set_attribute(ATTR_MAX_HEIGHT, Some(max_height));
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

/// Flows text glyphs along a vector path following the SVG 2 text-on-path layout rules (§11.8).
#[node_macro::node(category("Text"))]
fn text_on_path(
	_: impl Ctx,
	/// The text content to flow along the path.
	#[default("Lorem ipsum")]
	text: Item<String>,
	/// The vector path that glyphs follow.
	path: Item<Vector>,
	/// The loaded font file used to draw the text. The editor resolves the chosen typeface to these bytes via the resource system.
	font: Item<Resource>,
	/// The font size in pixels.
	#[unit(" px")]
	#[default(24.)]
	#[hard_min(1.)]
	size: Item<f64>,
	/// Additional spacing, in pixels, added between each character.
	#[unit(" px")]
	#[step(0.1)]
	character_spacing: Item<f64>,
	/// Arc-length offset from the path start to the first glyph.
	#[unit(" px")]
	start_offset: Item<f64>,
	/// If true, start_offset is treated as a 0–1 fraction of total path length.
	start_offset_percent: Item<bool>,
	/// Which side of the path direction to place text.
	side: Item<TextPathSide>,
	/// Text anchor point — affects where along the path the text is anchored.
	text_anchor: Item<TextAnchor>,
	/// Glyph rendering method. 'Align' uses rigid transforms; 'Stretch' warps glyphs along the path curvature.
	method: Item<TextPathMethod>,
	/// Spacing mode. 'Exact' uses computed positions; 'Auto' adjusts for path curvature.
	spacing: Item<TextPathSpacing>,
	/// Whether a forced text length is enabled.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_text_length: Item<bool>,
	/// If set, forces the total text advance to this length along the path.
	#[unit(" px")]
	#[hard_min(0.)]
	text_length: Item<f64>,
	/// How to fit text to the forced text length: adjust spacing only, or spacing and glyph widths.
	length_adjust: Item<LengthAdjust>,
	/// Whether a custom path authoring length is enabled.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_path_length: Item<bool>,
	/// Authoring path length for scaling startOffset. Maps the offset to the actual path length.
	#[unit(" px")]
	#[hard_min(0.)]
	path_length: Item<f64>,
	/// Right-to-left text direction.
	rtl: Item<bool>,
) -> List<Vector> {
	let path_list = List::new_from_item(Item::new_from_element(path.into_element()));
	text_nodes::text_on_path::place_text_on_path(
		text.element(),
		&path_list,
		font.element(),
		*size.element(),
		*character_spacing.element(),
		*start_offset.element(),
		*start_offset_percent.element(),
		*side.element(),
		*text_anchor.element(),
		*method.element(),
		*spacing.element(),
		(*has_text_length.element()).then_some(*text_length.element()),
		*length_adjust.element(),
		(*has_path_length.element()).then_some(*path_length.element()),
		*rtl.element(),
	)
}
