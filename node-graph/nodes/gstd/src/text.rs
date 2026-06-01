use core_types::Ctx;
use core_types::list::List;
use graph_craft::application_io::resource::{Resource, ResourceHash};
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

	#[data] cache: std::sync::Arc<std::sync::Mutex<Option<(ResourceHash, Blob<u8>)>>>,
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

	let font_blob = {
		let mut cache = cache.lock().unwrap();
		match cache.as_ref() {
			Some((cached_hash, cached_blob)) if *cached_hash == font.hash() => cached_blob.clone(),
			_ => {
				let new_blob = Blob::new((&font).into());
				*cache = Some((font.hash(), new_blob.clone()));
				new_blob
			}
		}
	};

	to_path(&text, &font_blob, typesetting, separate_glyphs)
}
