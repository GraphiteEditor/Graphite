use crate::vector::{VectorData, VectorDataTable};
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_core::Ctx;
pub use graphene_core::text::*;

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: impl Ctx,
	editor: &'i WasmEditorApi,
	text: String,
	font_name: Font,
	#[default(24.)] font_size: f64,
	#[default(1.2)] line_height_ratio: f64,
	#[default(0.)] character_spacing: f64,
	#[default(None)] max_width: Option<f64>,
	#[default(None)] max_height: Option<f64>,
	#[default(0.)] shear: f64,
) -> VectorDataTable {
	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
		shear,
	};

	let font_data = editor.font_cache.get(&font_name).map(|f| load_font(f));

	let result = VectorData::from_subpaths(to_path(&text, font_data, typesetting), false);

	VectorDataTable::new(result)
}
