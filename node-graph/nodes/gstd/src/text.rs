use core_types::attribute::{Attr, FontSize, LetterSpacing, LetterTilt, LineHeight, MaxHeight, MaxWidth, Transform as TransformAttr};
use core_types::extent::{LevelIn, ListIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::list::List;
use core_types::node::{Lane, RecordLane};
use core_types::{ATTR_TRANSFORM, Ctx, ExtractIndex, InjectIndex};
use glam::DAffine2;
use graph_craft::application_io::resource::Resource;
use graphic_types::Vector;
use text_nodes::markers::{Font as FontAttr, TextAlign as TextAlignAttr};
pub use text_nodes::*;

/// Produces a styled `String` carrying all typographic attributes.
///
/// Use the **Text to Vector** node to convert this into vector geometry if desired.
#[node_macro::node(category("Text"))]
fn text<'e>(
	ctx: impl Ctx + ExtractArena<'e>,
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
	#[hard(1..)]
	size: f64,
	/// The line height ratio, relative to the font size. Each line is drawn lower than its previous line by the distance of *Size* × *Line Height*.
	///
	/// 0 means all lines overlap. 1 means all lines are spaced by just the font size. 1.2 is a common default for readable text. 2 means double-spaced text.
	#[unit("x")]
	#[hard(0..)]
	#[step(0.1)]
	#[default(1.2)]
	line_height: f64,
	/// Additional spacing, in pixels, added between each character.
	#[unit(" px")]
	#[step(0.1)]
	letter_spacing: f64,
	/// The angle of faux italic slant applied to each glyph.
	#[unit("°")]
	#[hard(-85..85)]
	letter_tilt: f64,
	/// Enables the maximum width constraint so lines can wrap.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_width: bool,
	/// The maximum width that the text block can occupy before wrapping to a new line. Otherwise, lines do not wrap.
	#[unit(" px")]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_width: f64,
	/// Whether the *Max Height* property is enabled so that lines beyond it are not drawn.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_max_height: bool,
	/// The maximum height that the text block can occupy. Excess lines are not drawn.
	#[unit(" px")]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_height: f64,
	/// The horizontal alignment of each line of text within its surrounding box. To have an effect on a single line of text, *Max Width* must be set.
	#[widget(ParsedWidgetOverride::Custom = "text_align")]
	align: TextAlign,
) -> Result<
	(
		String,
		Attr<'e, FontAttr>,
		Attr<'e, FontSize>,
		Attr<'e, LineHeight>,
		Attr<'e, LetterSpacing>,
		Attr<'e, LetterTilt>,
		Attr<'e, MaxWidth>,
		Attr<'e, MaxHeight>,
		Attr<'e, TextAlignAttr>,
	),
	Interrupt,
> {
	let (font, _) = ctx.arena().alloc(font).ok_or_else(|| Interrupt::from(GraphError::new("the arena is exhausted")))?;
	Ok((
		text,
		Attr(font),
		Attr(size),
		Attr(line_height),
		Attr(letter_spacing),
		Attr(letter_tilt),
		Attr(has_max_width.then_some(max_width)),
		Attr(has_max_height.then_some(max_height)),
		Attr(align),
	))
}

/// Shapes one styled string lane into vector geometry, the font and
/// typesetting read from the lane's columns. The paths keep their glyph-local
/// transforms; the lane's own transform composes on at emit.
fn shape_lane(lane: &RecordLane<'_>, text: &str, separate_glyphs: bool) -> List<Vector> {
	if text.is_empty() {
		return List::new();
	}
	let font = lane.attr::<FontAttr>();
	let font = match font.is_empty() {
		true => &FALLBACK_FONT_RESOURCE,
		false => font,
	};
	let typesetting = TypesettingConfig {
		font_size: lane.attr::<FontSize>(),
		line_height_ratio: lane.attr::<LineHeight>(),
		letter_spacing: lane.attr::<LetterSpacing>(),
		letter_tilt: lane.attr::<LetterTilt>(),
		max_width: lane.attr::<MaxWidth>(),
		max_height: lane.attr::<MaxHeight>(),
		align: lane.attr::<TextAlignAttr>(),
	};
	to_path(text, font, typesetting, separate_glyphs)
}

/// The shaped paths of every string lane, valid for one key and generation,
/// so addressing the level's lanes shapes each string once.
#[derive(Debug, Default)]
pub struct ShapedRows {
	key: u64,
	generation: u64,
	rows: Vec<List<Vector>>,
}

type ShapedCache = std::sync::Arc<std::sync::Mutex<Option<ShapedRows>>>;

/// The lane-normalized cache key and arena generation of one evaluation.
macro_rules! eval_key {
	($ctx:expr) => {{
		let mut keyed = *$ctx;
		InjectIndex::set_index(&mut keyed, 0);
		(core_types::registry::cache_key(&keyed), $ctx.arena().generation())
	}};
}

/// The `lane`-th path over all the strings' shaped rows, carrying its
/// string's columns with the composed transform overriding. `key` and
/// `generation` scope the cache to one evaluation.
fn shaped_lane<'a, 'e>(
	strings: core_types::node::List<'a, String>,
	lane: usize,
	(key, generation): (u64, u64),
	cache: &ShapedCache,
	separate_glyphs: bool,
) -> Result<(Lane<'a, Vector>, Attr<'e, TransformAttr>), Interrupt> {
	let mut cached = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
	if !matches!(cached.as_ref(), Some(entry) if entry.key == key && entry.generation == generation) {
		let rows = (0..strings.len()).map(|row| shape_lane(&strings.lane(row), strings.element_ref(row), separate_glyphs)).collect();
		*cached = Some(ShapedRows { key, generation, rows });
	}
	let rows = &cached.as_ref().expect("populated above").rows;

	let mut remaining = lane;
	for (row, shaped) in rows.iter().enumerate() {
		if remaining >= shaped.len() {
			remaining -= shaped.len();
			continue;
		}
		let element = shaped.element(remaining).cloned().unwrap_or_default();
		let local: DAffine2 = shaped.attribute_cloned_or_default(ATTR_TRANSFORM, remaining);
		let carrier = strings.lane(row);
		let transform = carrier.attr::<TransformAttr>() * local;
		return Ok((carrier.map_element(element), Attr(transform)));
	}
	Err(GraphError::past_end().into())
}

/// The level holds every string's shaped paths in order.
fn shaped_extent(strings: ListIn<'_, String>, level: LevelIn, separate_glyphs: bool) -> GPoll<Extent> {
	match level.top() {
		true => strings
			.get()
			.map(|strings| Extent::Exactly((0..strings.len()).map(|row| shape_lane(&strings.lane(row), strings.element_ref(row), separate_glyphs).len()).sum())),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Converts styled text into vector compound paths, one per string.
#[node_macro::node(category("Text"), name("Text to Vector"), extent(text_to_vector_extent))]
fn text_to_vector<'e>(
	ctx: impl Ctx + core_types::CacheHash + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	/// Styled strings produced by the **Text** node (or any other `String` source).
	strings: IList<String>,
	#[data] shaped: ShapedCache,
) -> Result<IList<(Lane<Vector>, Attr<'e, TransformAttr>)>, Interrupt> {
	shaped_lane(strings, ctx.index() as usize, eval_key!(ctx), shaped, false)
}

fn text_to_vector_extent(strings: ListIn<'_, String>, level: LevelIn) -> GPoll<Extent> {
	shaped_extent(strings, level, false)
}

/// Splits styled text into a separate vector path for each of its glyphs (letterforms).
#[node_macro::node(category("Text"), name("Text to Vector Glyphs"), extent(text_to_vector_glyphs_extent))]
fn text_to_vector_glyphs<'e>(
	ctx: impl Ctx + core_types::CacheHash + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	/// Styled strings produced by the **Text** node (or any other `String` source).
	strings: IList<String>,
	#[data] shaped: ShapedCache,
) -> Result<IList<(Lane<Vector>, Attr<'e, TransformAttr>)>, Interrupt> {
	shaped_lane(strings, ctx.index() as usize, eval_key!(ctx), shaped, true)
}

fn text_to_vector_glyphs_extent(strings: ListIn<'_, String>, level: LevelIn) -> GPoll<Extent> {
	shaped_extent(strings, level, true)
}
