use core_types::list::List;
use core_types::{ATTR_FONT, ATTR_FONT_SIZE, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_TEXT_ALIGN, Ctx};
use graph_craft::application_io::resource::Resource;
use graphic_types::Vector;
pub use text_nodes::*;

const DEFAULT_FONT_SIZE: f64 = 24.;
const DEFAULT_LINE_HEIGHT: f64 = 1.2;

/// Produces a styled `String[]` carrying all typographic attributes.
#[node_macro::node(category("Text"))]
fn text(
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
	/// Additional spacing in document-space pixels added between every letter pair.
	#[unit(" px")]
	#[step(0.1)]
	letter_spacing: f64,
	/// Faux-italic slant angle in degrees applied to each letter.
	#[unit("°")]
	#[hard_min(-85.)]
	#[hard_max(85.)]
	letter_tilt: f64,
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
	/// Horizontal alignment of each line within the text block.
	#[widget(ParsedWidgetOverride::Custom = "text_align")]
	align: TextAlign,
) -> List<String> {
	let mut list = List::new_from_element(text);

	// Insert only when value deviates from its default as each stored attribute has runtime cost.

	if font != Resource::default() {
		list.set_attribute(ATTR_FONT, 0, font);
	}
	if (size - DEFAULT_FONT_SIZE).abs() > f64::EPSILON {
		list.set_attribute(ATTR_FONT_SIZE, 0, size);
	}
	if (line_height - DEFAULT_LINE_HEIGHT).abs() > f64::EPSILON {
		list.set_attribute(ATTR_LINE_HEIGHT, 0, line_height);
	}
	if letter_spacing != 0. {
		list.set_attribute(ATTR_LETTER_SPACING, 0, letter_spacing);
	}
	if letter_tilt != 0. {
		list.set_attribute(ATTR_LETTER_TILT, 0, letter_tilt);
	}
	if has_max_width {
		list.set_attribute(ATTR_MAX_WIDTH, 0, Some(max_width));
	}
	if has_max_height {
		list.set_attribute(ATTR_MAX_HEIGHT, 0, Some(max_height));
	}
	if align != TextAlign::default() {
		list.set_attribute(ATTR_TEXT_ALIGN, 0, align);
	}

	list
}

/// Converts a styled `String[]` into vector geometry.
/// Each string item is independently shaped by Parley and vectorised via skrifa.
#[node_macro::node(category("Text"), name("Text to Vector"))]
fn text_to_vector(
	_: impl Ctx,
	/// A styled list of text strings produced by the **Text** node (or any other `String[]` source).
	#[implementations(List<String>)]
	strings: List<String>,
	/// When enabled, each glyph is emitted as its own vector item instead of a single compound path per string.
	separate_glyphs: bool,
) -> List<Vector> {
	shape_text_list(&strings, separate_glyphs)
}
