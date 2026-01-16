use core_types::{Ctx, table::Table};
use graph_craft::wasm_application_io::WasmEditorApi;
use graphic_types::Vector;
pub use text_nodes::*;

#[node_macro::node(category("Text"))]
fn text<'i: 'n>(
	_: impl Ctx,
	#[scope("editor-api")] editor_resources: &'i WasmEditorApi,
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("Lorem ipsum")]
	text: String,
	#[widget(ParsedWidgetOverride::Custom = "text_font")] font: Font,
	#[unit(" px")]
	#[default(24.)]
	#[hard_min(1.)]
	size: f64,
	#[unit("x")]
	#[hard_min(0.)]
	#[step(0.1)]
	#[default(1.2)]
	line_height: f64,
	#[unit(" px")]
	#[step(0.1)]
	character_spacing: f64,
	#[widget(ParsedWidgetOverride::Hidden)] has_max_width: bool,
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_width: f64,
	#[widget(ParsedWidgetOverride::Hidden)] has_max_height: bool,
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	max_height: f64,
	#[unit("°")]
	#[hard_min(-85.)]
	#[hard_max(85.)]
	tilt: f64,
	#[widget(ParsedWidgetOverride::Custom = "text_align")] align: TextAlign,
	separate_glyph_elements: bool,
) -> Table<Vector> {
	let typesetting = TypesettingConfig {
		font_size: size,
		line_height_ratio: line_height,
		character_spacing,
		max_width: has_max_width.then_some(max_width),
		max_height: has_max_height.then_some(max_height),
		tilt,
		align,
	};

	to_path(&text, &font, &editor_resources.font_cache, typesetting, separate_glyph_elements)
}
