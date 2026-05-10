use core_types::Ctx;
use core_types::list::{Item, List};
use graph_craft::application_io::PlatformEditorApi;
use graphic_types::Vector;
pub use text_nodes::*;

/// Draws a text string as vector geometry with a choice of font and styling.
#[node_macro::node(category("Text"))]
fn text<'i: 'n>(
	_: impl Ctx,
	/// The Graphite editor's source for global font resources.
	#[scope("editor-api")]
	editor_resources: Item<&'i PlatformEditorApi>,
	/// The text content to be drawn.
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("Lorem ipsum")]
	text: Item<String>,
	/// The typeface used to draw the text.
	#[widget(ParsedWidgetOverride::Custom = "text_font")]
	font: Item<Font>,
	/// The font size used to draw the text.
	#[unit(" px")]
	#[default(24.)]
	#[hard_min(1.)]
	size: Item<f64>,
	/// The line height ratio, relative to the font size. Each line is drawn lower than its previous line by the distance of *Size* × *Line Height*.
	///
	/// 0 means all lines overlap. 1 means all lines are spaced by just the font size. 1.2 is a common default for readable text. 2 means double-spaced text.
	#[unit("x")]
	#[hard_min(0.)]
	#[step(0.1)]
	#[default(1.2)]
	line_height: Item<f64>,
	/// Additional spacing, in pixels, added between each character.
	#[unit(" px")]
	#[step(0.1)]
	character_spacing: Item<f64>,
	/// Whether the *Max Width* property is enabled so that lines can wrap to fit its specified block width.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_width: Item<bool>,
	/// The maximum width that the text block can occupy before wrapping to a new line. Otherwise, lines do not wrap.
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_width: Item<f64>,
	/// Whether the *Max Height* property is enabled so that lines beyond it are not drawn.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_height: Item<bool>,
	/// The maximum height that the text block can occupy. Excess lines are not drawn.
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_height: Item<f64>,
	/// The angle of faux italic slant applied to each glyph.
	#[unit("°")]
	#[hard_min(-85.)]
	#[hard_max(85.)]
	tilt: Item<f64>,
	/// The horizontal alignment of each line of text within its surrounding box.
	/// To have an effect on a single line of text, *Max Width* must be set.
	#[widget(ParsedWidgetOverride::Custom = "text_align")]
	align: Item<TextAlign>,
	/// Whether to split every letterform into its own vector item. Otherwise, a single vector compound path is produced.
	separate_glyphs: Item<bool>,
) -> Item<List<Vector>> {
	let editor_resources = editor_resources.into_element();
	let text = text.into_element();
	let font = font.into_element();
	let size = size.into_element();
	let line_height = line_height.into_element();
	let character_spacing = character_spacing.into_element();
	let has_max_width = has_max_width.into_element();
	let max_width = max_width.into_element();
	let has_max_height = has_max_height.into_element();
	let max_height = max_height.into_element();
	let tilt = tilt.into_element();
	let align = align.into_element();
	let separate_glyphs = separate_glyphs.into_element();

	let typesetting = TypesettingConfig {
		font_size: size,
		line_height_ratio: line_height,
		character_spacing,
		max_width: has_max_width.then_some(max_width),
		max_height: has_max_height.then_some(max_height),
		tilt,
		align,
	};

	Item::new_from_element(to_path(&text, &font, &editor_resources.font_cache, typesetting, separate_glyphs))
}
