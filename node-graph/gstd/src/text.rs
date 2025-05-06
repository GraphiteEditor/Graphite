use crate::vector::{VectorData, VectorDataTable};
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_core::text::TypesettingConfig;
pub use graphene_core::text::{Font, FontCache, bounding_box, load_face, to_group, to_path};
use graphene_core::{Ctx, GraphicGroupTable};

#[node_macro::node(category("Text"))] // Changed category for clarity
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
	#[default(false)] output_instances: bool, // Added parameter
) -> GraphicGroupTable {
	// Changed return type
	let buzz_face = editor.font_cache.get(&font_name).map(|data| load_face(data));

	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
	};

	if output_instances {
		to_group(&text, buzz_face, typesetting)
	} else {
		let vector_data = VectorData::from_subpaths(to_path(&text, buzz_face, typesetting), false);
		let vector_table = VectorDataTable::new(vector_data);
		// Convert VectorDataTable into GraphicGroupTable
		vector_table.into()
	}
}
