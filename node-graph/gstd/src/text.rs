use graph_craft::wasm_application_io::WasmEditorApi;
pub use graphene_core::text::*;
use graphene_core::{Ctx, table::Table, vector::Vector};

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: impl Ctx,
	editor: &'i WasmEditorApi,
	text: String,
	font_name: Font,
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
	/// Splits each text glyph into its own row in the table of vector geometry.
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

	to_path(&text, &font_name, &editor.font_cache, typesetting, per_glyph_instances)
}
