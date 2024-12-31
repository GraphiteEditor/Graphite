use graph_craft::wasm_application_io::WasmEditorApi;

use graphene_core::text::TypesettingConfiguration;
pub use graphene_core::text::{bounding_box, load_face, to_path, Font, FontCache};

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: (),
	editor: &'i WasmEditorApi,
	text: String,
	font_name: Font,
	#[default(24.)] font_size: f64,
	#[default(1.2)] line_height_ratio: f64,
	#[default(1.)] character_spacing: f64,
	#[default(None)] max_width: Option<f64>,
	#[default(None)] max_height: Option<f64>,
) -> crate::vector::VectorData {
	let buzz_face = editor.font_cache.get(&font_name).map(|data| load_face(data));

	let typesetting = TypesettingConfiguration {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
	};
	crate::vector::VectorData::from_subpaths(to_path(&text, buzz_face, typesetting), false)
}
