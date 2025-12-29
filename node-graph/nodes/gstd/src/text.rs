use core_types::{Ctx, table::Table};
use graph_craft::wasm_application_io::WasmEditorApi;
use graphic_types::Vector;
pub use text_nodes::*;

/// Converts text into editable vector shapes with customizable styling.
/// Parameters control font, size, spacing, alignment, and layout.

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: impl Ctx,
	editor: &'i WasmEditorApi,
	text: String,
	font: Font,
	#[unit(" px")]
	#[default(24.)]
	font_size: f64,
	#[unit("x")]
	#[default(1.2)]
	line_height_ratio: f64,
	#[unit(" px")]
	#[default(0.)]
	character_spacing: f64,
	#[unit(" px")] max_width: Option<f64>,
	#[unit(" px")] max_height: Option<f64>,
	/// Faux italic.
	#[unit("°")]
	#[default(0.)]
	tilt: f64,
	align: TextAlign,
    /// When disabled, outputs a single vector element containing all characters combined.
	/// When enabled, outputs a table with one vector element per character,
	/// allowing individual character manipulation by subsequent operations.
	#[default(false)]
	per_glyph_instances: bool,
) -> Table<Vector> {
	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
		tilt,
		align,
	};

	to_path(&text, &font, &editor.font_cache, typesetting, per_glyph_instances)
}
