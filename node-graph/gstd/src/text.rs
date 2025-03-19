use crate::vector::{VectorData, VectorDataTable};
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_core::Ctx;
use graphene_core::text::TypesettingConfig;
pub use graphene_core::text::{Font, FontCache, bounding_box, load_face, to_path};

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: impl Ctx,
	editor: &'i WasmEditorApi,
	text: String,
	font_name: Font,
	#[default(24.)] font_size: f64,
	#[default(1.2)] line_height_ratio: f64,
	#[default(1.)] character_spacing: f64,
	#[default(None)] max_width: Option<f64>,
	#[default(None)] max_height: Option<f64>,
) -> VectorDataTable {
	let buzz_face = editor.font_cache.get(&font_name).map(|data| load_face(data));

	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
	};

	let result = VectorData::from_subpaths(to_path(&text, buzz_face, typesetting), false);

	VectorDataTable::new(result)
}
